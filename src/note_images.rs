use anyhow::{Context, Result, ensure};
use image::{ImageFormat, ImageReader};
use std::{
    io::{Cursor, Read},
    path::{Path, PathBuf},
};

pub const MAX_BYTES: u64 = 20 * 1024 * 1024;

pub enum Source {
    File(PathBuf),
    Bytes(Vec<u8>),
}

pub fn clipboard_source(item: &gpui_kit::ClipboardItem) -> Result<Option<Source>> {
    use gpui_kit::ClipboardEntry;
    for entry in item.entries() {
        if let ClipboardEntry::Image(image) = entry {
            ensure!(
                image.bytes().len() as u64 <= MAX_BYTES,
                "Choose an image smaller than 20 MB"
            );
            return Ok(Some(Source::Bytes(image.bytes().to_vec())));
        }
    }
    Ok(item.entries().iter().find_map(|entry| match entry {
        ClipboardEntry::ExternalPaths(paths) => paths
            .paths()
            .iter()
            .find(|path| {
                path.extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| {
                        matches!(
                            ext.to_ascii_lowercase().as_str(),
                            "png" | "jpg" | "jpeg" | "webp" | "bmp"
                        )
                    })
            })
            .cloned()
            .map(Source::File),
        _ => None,
    }))
}

pub fn import(source: Source, directory: &Path) -> Result<String> {
    match source {
        Source::File(path) => import_path(&path, directory),
        Source::Bytes(bytes) => import_bytes(&bytes, directory),
    }
}

pub fn import_path(path: &Path, directory: &Path) -> Result<String> {
    let file = std::fs::File::open(path).context("Couldn't open the image")?;
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
    import_bytes(&bytes, directory)
}

/// Decode within limits and save an independent PNG, so notes survive moves
/// or deletion of the original image and do not carry arbitrary file payloads.
pub fn import_bytes(bytes: &[u8], directory: &Path) -> Result<String> {
    ensure!(
        !bytes.is_empty() && bytes.len() as u64 <= MAX_BYTES,
        "Choose an image smaller than 20 MB"
    );
    let mut reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    ensure!(
        matches!(
            reader.format(),
            Some(ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP | ImageFormat::Bmp)
        ),
        "Use a PNG, JPEG, WebP or BMP image"
    );
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode().context("Couldn't decode this image")?;
    let assets = directory.join("note-images");
    std::fs::create_dir_all(&assets)?;
    let name = format!("{}.png", uuid::Uuid::new_v4());
    let path = assets.join(&name);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    let result = image
        .write_to(&mut file, ImageFormat::Png)
        .and_then(|_| file.sync_all().map_err(Into::into));
    drop(file);
    if let Err(error) = result {
        let _ = std::fs::remove_file(path);
        return Err(error.into());
    }
    Ok(format!("nen-image://{name}"))
}

pub fn resolve(uri: &str, directory: &Path) -> Option<PathBuf> {
    let name = uri.strip_prefix("nen-image://")?;
    let id = uuid::Uuid::parse_str(name.strip_suffix(".png")?).ok()?;
    if name != format!("{id}.png") {
        return None;
    }
    let assets = directory.join("note-images").canonicalize().ok()?;
    let path = assets.join(name).canonicalize().ok()?;
    (path.parent() == Some(assets.as_path()) && path.is_file()).then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn managed_images_survive_original_removal_and_reject_external_references() -> Result<()> {
        let root = std::env::temp_dir().join(format!("nen-note-image-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root)?;
        let original = root.join("original.png");
        image::RgbaImage::from_pixel(20, 10, image::Rgba([30, 90, 180, 255])).save(&original)?;
        let uri = import_path(&original, &root)?;
        std::fs::remove_file(original)?;
        let path = resolve(&uri, &root).expect("managed image");
        assert_eq!(image::open(&path)?.width(), 20);
        for uri in [
            "https://example.com/image.png",
            "file:///C:/secret.png",
            "nen-image://../original.png",
            "nen-image://%2e%2e/original.png",
        ] {
            assert!(resolve(uri, &root).is_none());
        }
        assert!(import_bytes(b"not an image", &root).is_err());
        std::fs::remove_file(path)?;
        std::fs::remove_dir(root.join("note-images"))?;
        std::fs::remove_dir(root)?;
        Ok(())
    }
}
