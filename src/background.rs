use crate::models::Settings;
use anyhow::{Result, bail};
use image::{GenericImageView, ImageReader};
use std::path::{Path, PathBuf};

pub fn prepare(settings: &Settings, directory: &Path) -> Result<Option<PathBuf>> {
    let Some(path) = settings.background_image.as_ref() else {
        return Ok(None);
    };
    if std::fs::metadata(path)?.len() > 100 * 1024 * 1024 {
        bail!("Choose an image smaller than 100 MB.");
    }
    let mut reader = ImageReader::open(path)?.with_guessed_format()?;
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
    let blur = if settings.surface == "Frosted" {
        settings.background_blur.max(12.)
    } else {
        settings.background_blur
    };
    if blur > 0.1 {
        image = image.blur(blur.clamp(0., 40.));
    }
    let mut pixels = image.to_rgba8();
    let saturation = settings.background_saturation.clamp(0., 2.);
    for pixel in pixels.pixels_mut() {
        let gray = 0.2126 * pixel[0] as f32 + 0.7152 * pixel[1] as f32 + 0.0722 * pixel[2] as f32;
        for channel in 0..3 {
            pixel[channel] =
                (gray + (pixel[channel] as f32 - gray) * saturation).clamp(0., 255.) as u8;
        }
    }
    let assets = directory.join("backgrounds");
    std::fs::create_dir_all(&assets)?;
    let path = assets.join(format!("render-{}.png", uuid::Uuid::new_v4()));
    pixels.save(&path)?;
    Ok(Some(path))
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
