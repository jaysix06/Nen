use super::{Update, file_hash, verify_executable};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

const FILES: &[&str] = &[
    "download.exe",
    "updater.exe",
    "backup.exe",
    "plan.json",
    "ready",
    "commit",
    "error.txt",
];

#[derive(Serialize, Deserialize)]
struct Plan {
    target: PathBuf,
    original_sha256: String,
    sha256: String,
    size: u64,
}

pub struct PreparedUpdate {
    directory: PathBuf,
    target: PathBuf,
    keep: bool,
}

impl PreparedUpdate {
    pub(super) fn new() -> Result<Self> {
        let target = std::env::current_exe()?.canonicalize()?;
        ensure!(
            target
                .file_name()
                .is_some_and(|name| name.eq_ignore_ascii_case("Nen.exe")),
            "Updates can only be installed by the Nen application"
        );
        let directory = target
            .parent()
            .context("Nen's install directory is unavailable")?
            .join(format!(".nen-update-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&directory)
            .context("Nen's folder is not writable. Move Nen to a writable folder to update.")?;
        Ok(Self {
            directory,
            target,
            keep: false,
        })
    }

    pub(super) fn download_path(&self) -> PathBuf {
        self.directory.join("download.exe")
    }

    pub(super) fn write_plan(&self, update: &Update) -> Result<()> {
        let plan = Plan {
            target: self.target.clone(),
            original_sha256: file_hash(&self.target)?,
            sha256: update.sha256.clone(),
            size: update.size,
        };
        fs::write(self.directory.join("plan.json"), serde_json::to_vec(&plan)?)?;
        Ok(())
    }

    /// The trusted current executable runs a helper mode, without opening storage or GPUI.
    pub fn launch(self) -> Result<Installer> {
        let helper = self.directory.join("updater.exe");
        fs::copy(&self.target, &helper)?;
        let mut command = Command::new(&helper);
        command
            .arg("--apply-update")
            .arg(std::process::id().to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(windows::Win32::System::Threading::CREATE_NO_WINDOW.0);
        }
        let child = command
            .spawn()
            .context("Couldn't start Nen's update helper")?;
        let mut installer = Installer {
            child,
            prepared: self,
        };
        let deadline = Instant::now() + Duration::from_secs(10);
        while !installer.prepared.directory.join("ready").exists() {
            ensure!(
                installer.child.try_wait()?.is_none(),
                "Nen's update helper could not initialize"
            );
            ensure!(
                Instant::now() < deadline,
                "Nen's update helper did not respond"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
        Ok(installer)
    }
}

impl Drop for PreparedUpdate {
    fn drop(&mut self) {
        if !self.keep {
            remove_staging_files(&self.directory);
        }
    }
}

pub struct Installer {
    child: Child,
    prepared: PreparedUpdate,
}

impl Installer {
    pub fn cancel(&mut self) {
        self.prepared.keep = false;
    }

    pub fn commit(&mut self) -> Result<()> {
        fs::write(self.prepared.directory.join("commit"), b"install")?;
        self.prepared.keep = true;
        Ok(())
    }
}

impl Drop for Installer {
    fn drop(&mut self) {
        if !self.prepared.keep {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn read_plan(directory: &Path) -> Result<Plan> {
    let mut bytes = Vec::new();
    fs::File::open(directory.join("plan.json"))?
        .take(16 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 16 * 1024, "Invalid update plan");
    Ok(serde_json::from_slice(&bytes)?)
}

fn staging_directory(directory: &Path, target: &Path) -> Result<PathBuf> {
    let directory = directory.canonicalize()?;
    let name = directory
        .file_name()
        .and_then(|name| name.to_str())
        .context("Invalid update folder")?;
    uuid::Uuid::parse_str(
        name.strip_prefix(".nen-update-")
            .context("Invalid update folder")?,
    )?;
    ensure!(
        directory.parent() == target.parent(),
        "Update folder must be inside Nen's install directory"
    );
    Ok(directory)
}

fn remove_staging_files(directory: &Path) {
    // Only remove the known files in this unique, verified directory. Never recurse.
    for name in FILES {
        let _ = fs::remove_file(directory.join(name));
    }
    let _ = fs::remove_dir(directory);
}

/// The reopened app cleans up its own helper after Windows releases the executable.
pub fn cleanup_update(directory: &Path) -> Result<()> {
    let target = std::env::current_exe()?.canonicalize()?;
    let directory = staging_directory(directory, &target)?;
    let plan = read_plan(&directory)?;
    ensure!(
        plan.target.canonicalize()? == target,
        "Update belongs to a different application"
    );
    let hash = file_hash(&target)?;
    ensure!(
        hash == plan.sha256 || hash == plan.original_sha256,
        "Update cleanup identity does not match Nen"
    );
    for _ in 0..20 {
        remove_staging_files(&directory);
        if !directory.exists() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    anyhow::bail!("Could not remove Nen's update helper");
}

#[cfg(windows)]
pub fn apply_update(parent_pid: u32) -> Result<()> {
    use windows::{
        Win32::{Foundation::*, System::Threading::*},
        core::PWSTR,
    };
    let helper = std::env::current_exe()?.canonicalize()?;
    ensure!(
        helper.file_name().is_some_and(|name| name == "updater.exe"),
        "Invalid update helper"
    );
    let directory = helper.parent().context("Update folder is unavailable")?;
    let plan = read_plan(directory)?;
    let target = plan.target.canonicalize()?;
    let directory = staging_directory(directory, &target)?;
    ensure!(
        helper == directory.join("updater.exe")
            && target
                .file_name()
                .is_some_and(|name| name.eq_ignore_ascii_case("Nen.exe")),
        "Invalid update target"
    );
    ensure!(
        file_hash(&helper)? == plan.original_sha256 && file_hash(&target)? == plan.original_sha256,
        "The update helper does not match the installed Nen executable"
    );
    let download = directory.join("download.exe");
    ensure!(
        download.canonicalize()?.parent() == Some(directory.as_path()),
        "Invalid update download path"
    );
    verify_executable(&download, plan.size, &plan.sha256)?;
    // Hold the exact process handle before signalling readiness, avoiding PID reuse races.
    let process = unsafe {
        OpenProcess(
            PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
            false,
            parent_pid,
        )?
    };
    let wait = (|| -> Result<()> {
        let mut image = vec![0u16; 32768];
        let mut length = image.len() as u32;
        unsafe {
            QueryFullProcessImageNameW(
                process,
                PROCESS_NAME_WIN32,
                PWSTR(image.as_mut_ptr()),
                &mut length,
            )?
        };
        let parent =
            PathBuf::from(String::from_utf16(&image[..length as usize])?).canonicalize()?;
        ensure!(parent == target, "The update parent is not Nen");
        fs::write(directory.join("ready"), b"ready")?;
        ensure!(
            unsafe { WaitForSingleObject(process, 300_000) } == WAIT_OBJECT_0,
            "Nen did not finish saving and exit in time"
        );
        ensure!(
            directory.join("commit").is_file(),
            "Update was cancelled before shutdown"
        );
        Ok(())
    })();
    unsafe {
        let _ = CloseHandle(process);
    }
    wait?;
    // Recheck after waiting, since the staged download is writable by the current user.
    let install = (|| -> Result<()> {
        ensure!(
            file_hash(&target)? == plan.original_sha256,
            "Nen changed while the update was pending"
        );
        verify_executable(&download, plan.size, &plan.sha256)?;
        replace_and_relaunch(&target, &directory, |failed| {
            let mut command = Command::new(&target);
            command
                .current_dir(target.parent().context("Install folder is unavailable")?)
                .arg("--cleanup-update")
                .arg(&directory);
            if failed {
                command.arg("--update-failed");
            }
            command.spawn().context("Couldn't reopen Nen")?;
            Ok(())
        })
    })();
    if let Err(error) = install {
        let _ = fs::write(directory.join("error.txt"), error.to_string());
        // If validation failed before replacement, the original app is still usable.
        if file_hash(&target).is_ok_and(|hash| hash == plan.original_sha256) {
            let _ = Command::new(&target)
                .arg("--update-failed")
                .arg("--cleanup-update")
                .arg(&directory)
                .spawn();
        }
        return Err(error);
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn apply_update(_: u32) -> Result<()> {
    anyhow::bail!("Nen updates are only supported on Windows");
}

fn rename_with_retry(from: &Path, to: &Path) -> Result<()> {
    for attempt in 0..30 {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(error) if attempt == 29 => return Err(error.into()),
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
    unreachable!()
}

fn replace_and_relaunch(
    target: &Path,
    directory: &Path,
    mut launch: impl FnMut(bool) -> Result<()>,
) -> Result<()> {
    let backup = directory.join("backup.exe");
    rename_with_retry(target, &backup)?;
    let result =
        rename_with_retry(&directory.join("download.exe"), target).and_then(|_| launch(false));
    if let Err(error) = result {
        if target.exists() {
            fs::remove_file(target)?;
        }
        rename_with_retry(&backup, target)
            .context("Could not restore Nen's previous executable")?;
        return Err(error);
    }
    let _ = fs::remove_file(backup);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacement_reopens_new_binary_and_rolls_back_when_launch_fails() -> Result<()> {
        let root = std::env::temp_dir().join(format!("nen-install-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root)?;
        let target = root.join("Nen.exe");
        for fail in [false, true] {
            let directory = root.join(format!(".nen-update-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&directory)?;
            fs::write(&target, b"previous executable")?;
            fs::write(directory.join("download.exe"), b"new executable")?;
            let result = replace_and_relaunch(&target, &directory, |_| {
                assert_eq!(fs::read(&target)?, b"new executable");
                if fail {
                    anyhow::bail!("Simulated process launch failure");
                }
                Ok(())
            });
            assert_eq!(result.is_err(), fail);
            assert_eq!(
                fs::read(&target)?,
                if fail {
                    b"previous executable".as_slice()
                } else {
                    b"new executable".as_slice()
                }
            );
            remove_staging_files(&directory);
        }
        fs::remove_file(target)?;
        fs::remove_dir(root)?;
        Ok(())
    }

    #[test]
    fn staging_must_belong_to_the_install_directory() -> Result<()> {
        let root = std::env::temp_dir().join(format!("nen-boundary-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root)?;
        let directory = root.join(format!(".nen-update-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&directory)?;
        let target = root.canonicalize()?.join("Nen.exe");
        assert!(staging_directory(&directory, &target).is_ok());
        assert!(staging_directory(&directory, &root.join("elsewhere/Nen.exe")).is_err());
        assert!(staging_directory(&root, &target).is_err());
        fs::remove_dir(directory)?;
        fs::remove_dir(root)?;
        Ok(())
    }
}
