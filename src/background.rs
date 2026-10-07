use crate::models::Settings;
use anyhow::{Result, bail};
use image::{GenericImageView, ImageReader};
use std::path::{Path, PathBuf};

pub fn prepare(settings: &Settings, directory: &Path) -> Result<Option<PathBuf>> {
    let assets = directory.join("backgrounds");
    std::fs::create_dir_all(&assets)?;
    let path = if let Some(path) = &settings.background_image {
        PathBuf::from(path)
    } else if settings.default_wallpaper {
        let path = assets.join("nen-lake-v1.png");
        if !path.exists() {
            std::fs::write(&path, include_bytes!("../assets/nen-lake.png"))?;
        }
        path
    } else {
        return Ok(None);
    };
    let metadata = std::fs::metadata(&path)?;
    if metadata.len() > 100 * 1024 * 1024 {
        bail!("Choose an image smaller than 100 MB.");
    }
    let blur = if settings.surface == "Frosted" {
        settings.background_blur.max(2.)
    } else {
        settings.background_blur
    }
    .clamp(0., 40.);
    let saturation = settings.background_saturation.clamp(0., 2.);
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut hash);
    metadata.len().hash(&mut hash);
    metadata.modified()?.hash(&mut hash);
    blur.to_bits().hash(&mut hash);
    saturation.to_bits().hash(&mut hash);
    let cache = assets.join(format!("render-v2-{:x}.png", hash.finish()));
    if cache.exists() {
        return Ok(Some(cache));
    }
    let mut reader = ImageReader::open(&path)?.with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let mut image = reader.decode()?;
    let (w, h) = image.dimensions();
    if w > 2560 || h > 1600 {
        image = image.resize(2560, 1600, image::imageops::FilterType::Triangle);
    }
    if blur > 0.1 {
        image = image.blur(blur.clamp(0., 40.));
    }
    let mut pixels = image.to_rgba8();
    for pixel in pixels.pixels_mut() {
        let gray = 0.2126 * pixel[0] as f32 + 0.7152 * pixel[1] as f32 + 0.0722 * pixel[2] as f32;
        for channel in 0..3 {
            pixel[channel] =
                (gray + (pixel[channel] as f32 - gray) * saturation).clamp(0., 255.) as u8;
        }
    }
    let pending = assets.join(format!("pending-{}.png", uuid::Uuid::new_v4()));
    pixels.save(&pending)?;
    if cache.exists() {
        std::fs::remove_file(pending)?;
    } else {
        std::fs::rename(pending, &cache)?;
    }
    Ok(Some(cache))
}

pub fn import(path: &Path, directory: &Path) -> Result<PathBuf> {
    // Decode with resource limits before copying a user-selected file into app-managed storage.
    let assets = directory.join("backgrounds");
    std::fs::create_dir_all(&assets)?;
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("image");
    let target = assets.join(format!("source-{}.{}", uuid::Uuid::new_v4(), extension));
    let config = Settings {
        background_image: Some(path.to_string_lossy().into()),
        ..Default::default()
    };
    if let Some(preview) = prepare(&config, directory)? {
        std::fs::remove_file(preview)?;
    }
    std::fs::copy(path, &target)?;
    Ok(target)
}

/// Sample the rendered wallpaper once off the UI thread. Group nearby colors
/// so JPEG noise does not outweigh a large area of the same color.
pub fn dominant_color(path: &Path) -> Result<u32> {
    let mut reader = ImageReader::open(path)?.with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(256 * 1024 * 1024);
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    reader.limits(limits);
    let sample = reader.decode()?.thumbnail(64, 64).to_rgba8();
    let mut buckets = std::collections::HashMap::<u16, (u32, [u32; 3])>::new();
    for pixel in sample.pixels().filter(|pixel| pixel[3] >= 128) {
        let key =
            ((pixel[0] as u16 >> 4) << 8) | ((pixel[1] as u16 >> 4) << 4) | (pixel[2] as u16 >> 4);
        let (count, total) = buckets.entry(key).or_default();
        *count += 1;
        for channel in 0..3 {
            total[channel] += pixel[channel] as u32;
        }
    }
    let (_, (count, total)) = buckets
        .into_iter()
        .max_by_key(|(key, (count, _))| (*count, *key))
        .ok_or_else(|| anyhow::anyhow!("The wallpaper is fully transparent"))?;
    Ok(((total[0] / count) << 16) | ((total[1] / count) << 8) | (total[2] / count))
}
