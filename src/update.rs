//! GitHub release checks. Notes and settings never enter update requests.
mod install;
pub use install::{Installer, PreparedUpdate, apply_update, cleanup_update};

use anyhow::{Context, Result, ensure};
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::Path, time::Duration};
use url::Url;

const MAX_METADATA: u64 = 1024 * 1024;
const MAX_DOWNLOAD: u64 = 200 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct Update {
    pub version: String,
    pub download_url: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    size: u64,
    digest: Option<String>,
    state: String,
}

pub fn repository() -> Result<Option<String>> {
    let source = option_env!("NEN_UPDATE_REPOSITORY").unwrap_or(env!("CARGO_PKG_REPOSITORY"));
    if source.is_empty() {
        return Ok(None);
    }
    let url = Url::parse(source).context("Use an HTTPS GitHub repository URL")?;
    ensure!(
        url.scheme() == "https"
            && url.host_str() == Some("github.com")
            && url.username().is_empty()
            && url.password().is_none()
            && url.port().is_none()
            && url.query().is_none()
            && url.fragment().is_none(),
        "Use an HTTPS GitHub repository URL"
    );
    let path = url.path().trim_matches('/').trim_end_matches(".git");
    let parts: Vec<_> = path.split('/').collect();
    ensure!(
        parts.len() == 2
            && parts.iter().all(|part| {
                !part.is_empty()
                    && !matches!(*part, "." | "..")
                    && part
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
            }),
        "Use a GitHub owner/repository URL"
    );
    Ok(Some(path.to_owned()))
}

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .https_only(true)
        .max_redirects(5)
        .timeout_global(Some(timeout))
        .timeout_connect(Some(Duration::from_secs(10)))
        .user_agent(concat!("Nen/", env!("CARGO_PKG_VERSION")))
        .build()
        .new_agent()
}

/// Called once by the real application, never by AppState constructors in UI tests.
pub fn check() -> Result<Option<Update>> {
    let Some(repository) = repository()? else {
        return Ok(None);
    };
    let mut response = match agent(Duration::from_secs(15))
        .get(format!(
            "https://api.github.com/repos/{repository}/releases/latest"
        ))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .call()
    {
        Ok(response) => response,
        Err(ureq::Error::StatusCode(404)) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(MAX_METADATA + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= MAX_METADATA,
        "Release metadata is too large"
    );
    select_release(&bytes, &repository, env!("CARGO_PKG_VERSION"))
}

fn select_release(bytes: &[u8], repository: &str, current: &str) -> Result<Option<Update>> {
    let release: Release = serde_json::from_slice(bytes)?;
    let mut version = Version::parse(
        release
            .tag_name
            .strip_prefix('v')
            .unwrap_or(&release.tag_name),
    )?;
    let mut current = Version::parse(current)?;
    version.build = semver::BuildMetadata::EMPTY;
    current.build = semver::BuildMetadata::EMPTY;
    if release.draft || release.prerelease || !version.pre.is_empty() || version <= current {
        return Ok(None);
    }
    let Some(asset) = release
        .assets
        .into_iter()
        .find(|asset| asset.name == "Nen.exe" && asset.state == "uploaded")
    else {
        return Ok(None);
    };
    validate_download_url(&asset.browser_download_url, repository)?;
    ensure!(
        (1..=MAX_DOWNLOAD).contains(&asset.size),
        "Update size is invalid"
    );
    let digest = asset
        .digest
        .as_deref()
        .and_then(|value| value.strip_prefix("sha256:"))
        .context("The release executable needs GitHub's SHA-256 digest")?;
    ensure!(
        digest.len() == 64 && digest.bytes().all(|c| c.is_ascii_hexdigit()),
        "Update checksum is invalid"
    );
    Ok(Some(Update {
        version: version.to_string(),
        download_url: asset.browser_download_url,
        size: asset.size,
        sha256: digest.to_ascii_lowercase(),
    }))
}

fn validate_download_url(value: &str, repository: &str) -> Result<()> {
    let url = Url::parse(value)?;
    ensure!(
        url.scheme() == "https"
            && url.host_str() == Some("github.com")
            && url.username().is_empty()
            && url.password().is_none()
            && url.port().is_none()
            && url.query().is_none()
            && url.fragment().is_none()
            && url
                .path()
                .starts_with(&format!("/{repository}/releases/download/"))
            && url.path().ends_with("/Nen.exe"),
        "Update does not belong to Nen's GitHub repository"
    );
    Ok(())
}

pub fn prepare(update: &Update) -> Result<PreparedUpdate> {
    let repository = repository()?.context("No update repository is configured")?;
    validate_download_url(&update.download_url, &repository)?;
    ensure!(
        (1..=MAX_DOWNLOAD).contains(&update.size),
        "Update size is invalid"
    );
    let prepared = PreparedUpdate::new()?;
    let mut response = agent(Duration::from_secs(180))
        .get(&update.download_url)
        .call()?;
    let mut file = File::create(prepared.download_path())?;
    let copied = std::io::copy(
        &mut response.body_mut().as_reader().take(update.size + 1),
        &mut file,
    )?;
    ensure!(
        copied == update.size,
        "The update download is incomplete or has an unexpected size"
    );
    file.sync_all()?;
    drop(file);
    verify_executable(&prepared.download_path(), update.size, &update.sha256)?;
    prepared.write_plan(update)?;
    Ok(prepared)
}

fn file_hash(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hash = Sha256::new();
    let mut bytes = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut bytes)?;
        if count == 0 {
            break;
        }
        hash.update(&bytes[..count]);
    }
    use std::fmt::Write;
    let mut digest = String::with_capacity(64);
    for byte in hash.finalize() {
        write!(&mut digest, "{byte:02x}")?;
    }
    Ok(digest)
}

fn verify_executable(path: &Path, size: u64, digest: &str) -> Result<()> {
    use std::io::{Seek, SeekFrom};
    ensure!(
        std::fs::metadata(path)?.len() == size,
        "Update size verification failed"
    );
    ensure!(
        file_hash(path)? == digest,
        "Update checksum verification failed"
    );
    let mut file = File::open(path)?;
    let mut header = [0u8; 64];
    file.read_exact(&mut header)?;
    ensure!(
        &header[..2] == b"MZ",
        "The update is not a Windows executable"
    );
    let offset = u32::from_le_bytes(header[60..64].try_into()?) as u64;
    ensure!(
        offset >= 64 && offset + 6 <= size,
        "The executable header is invalid"
    );
    file.seek(SeekFrom::Start(offset))?;
    let mut pe = [0u8; 6];
    file.read_exact(&mut pe)?;
    ensure!(
        &pe[..4] == b"PE\0\0" && pe[4..] == [0x64, 0x86],
        "The update must be a Windows x64 executable"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str) -> serde_json::Value {
        serde_json::json!({
            "tag_name": tag, "draft": false, "prerelease": false,
            "assets": [{"name": "Nen.exe", "state": "uploaded", "size": 128,
                "browser_download_url": "https://github.com/owner/nen/releases/download/v1.0.0/Nen.exe",
                "digest": format!("sha256:{}", "a".repeat(64))}]
        })
    }

    fn select(value: &serde_json::Value, current: &str) -> Result<Option<Update>> {
        select_release(&serde_json::to_vec(value)?, "owner/nen", current)
    }

    #[test]
    fn only_newer_stable_releases_are_offered() {
        assert_eq!(
            select(&release("v0.10.0"), "0.9.9")
                .unwrap()
                .unwrap()
                .version,
            "0.10.0"
        );
        for tag in ["v0.9.9", "v0.9.8", "v0.9.9+build.2", "v1.0.0-beta.1"] {
            assert!(select(&release(tag), "0.9.9").unwrap().is_none());
        }
        for flag in ["draft", "prerelease"] {
            let mut value = release("v1.0.0");
            value[flag] = true.into();
            assert!(select(&value, "0.9.9").unwrap().is_none());
        }
        let mut value = release("v1.0.0");
        value["assets"] = serde_json::json!([]);
        assert!(select(&value, "0.9.9").unwrap().is_none());
    }

    #[test]
    fn untrusted_urls_sizes_and_missing_checksums_are_rejected() {
        for url in [
            "http://github.com/owner/nen/releases/download/v1/Nen.exe",
            "https://github.com.evil.test/owner/nen/releases/download/v1/Nen.exe",
            "https://github.com/other/nen/releases/download/v1/Nen.exe",
            "https://github.com/owner/nen/releases/download/v1/Nen.exe?redirect=bad",
            "https://github.com@evil.test/owner/nen/releases/download/v1/Nen.exe",
        ] {
            let mut value = release("v1.0.0");
            value["assets"][0]["browser_download_url"] = url.into();
            assert!(select(&value, "0.9.9").is_err(), "{url}");
        }
        for size in [0, MAX_DOWNLOAD + 1] {
            let mut value = release("v1.0.0");
            value["assets"][0]["size"] = size.into();
            assert!(select(&value, "0.9.9").is_err());
        }
        for digest in [
            serde_json::Value::Null,
            "sha256:abc".into(),
            format!("sha256:{}", "g".repeat(64)).into(),
        ] {
            let mut value = release("v1.0.0");
            value["assets"][0]["digest"] = digest;
            assert!(select(&value, "0.9.9").is_err());
        }
    }

    #[test]
    fn corrupt_and_wrong_architecture_downloads_are_rejected() -> Result<()> {
        let path = std::env::temp_dir().join(format!("nen-update-{}.exe", uuid::Uuid::new_v4()));
        let mut bytes = vec![0u8; 128];
        bytes[..2].copy_from_slice(b"MZ");
        bytes[60..64].copy_from_slice(&64u32.to_le_bytes());
        bytes[64..70].copy_from_slice(b"PE\0\0\x64\x86");
        std::fs::write(&path, &bytes)?;
        let hash = file_hash(&path)?;
        verify_executable(&path, 128, &hash)?;
        assert!(verify_executable(&path, 128, &"0".repeat(64)).is_err());
        assert!(verify_executable(&path, 127, &hash).is_err());
        bytes[68..70].copy_from_slice(&0x014cu16.to_le_bytes());
        std::fs::write(&path, &bytes)?;
        assert!(verify_executable(&path, 128, &file_hash(&path)?).is_err());
        std::fs::remove_file(path)?;
        Ok(())
    }
}
