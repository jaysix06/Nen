use nen::{background, models::Settings};

#[test]
fn wallpaper_cache_preserves_detail_and_tracks_image_effects() -> anyhow::Result<()> {
    let root = std::env::temp_dir().join(format!("nen-colors-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root)?;
    let path = root.join("original.png");
    let mut image = image::RgbaImage::from_pixel(64, 64, image::Rgba([210, 70, 30, 255]));
    for (x, _, pixel) in image.enumerate_pixels_mut() {
        if x < 16 {
            *pixel = image::Rgba([20, 40, 220, 255]);
        }
    }
    image.save(&path)?;
    let mut settings = Settings {
        background_image: Some(path.to_string_lossy().into()),
        surface: "Clear".into(),
        background_blur: 0.,
        background_dim: 0.,
        ..Default::default()
    };
    let original = background::prepare(&settings, &root)?.expect("wallpaper");
    let pixels = image::open(&original)?.to_rgba8();
    assert_eq!(pixels.get_pixel(4, 32).0, [20, 40, 220, 255]);
    assert_eq!(pixels.get_pixel(60, 32).0, [210, 70, 30, 255]);
    settings.background_dim = 0.8;
    assert_eq!(
        background::prepare(&settings, &root)?,
        Some(original.clone())
    );
    settings.surface = "Frosted".into();
    let frosted = background::prepare(&settings, &root)?.expect("frosted");
    assert_ne!(original, frosted);
    settings.background_blur = 12.;
    let blurred = background::prepare(&settings, &root)?.expect("blurred");
    assert_ne!(blurred, frosted);
    assert_ne!(
        image::open(blurred)?.to_rgba8().get_pixel(15, 32),
        pixels.get_pixel(15, 32)
    );
    settings.background_saturation = 0.;
    let gray =
        image::open(background::prepare(&settings, &root)?.expect("desaturated"))?.to_rgba8();
    let pixel = gray.get_pixel(32, 32);
    assert_eq!(pixel[0], pixel[1]);
    assert_eq!(pixel[1], pixel[2]);
    std::fs::remove_file(path)?;
    for entry in std::fs::read_dir(root.join("backgrounds"))? {
        std::fs::remove_file(entry?.path())?;
    }
    std::fs::remove_dir(root.join("backgrounds"))?;
    std::fs::remove_dir(root)?;
    Ok(())
}

#[test]
fn importing_and_transforming_a_background_never_changes_the_original() -> anyhow::Result<()> {
    let root = std::env::temp_dir().join(format!("nen-image-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root)?;
    let original = root.join("original.png");
    image::RgbaImage::from_pixel(64, 32, image::Rgba([180, 70, 30, 255])).save(&original)?;
    let bytes = std::fs::read(&original)?;
    let imported = background::import(&original, &root)?;
    assert_eq!(std::fs::read(&imported)?, bytes);
    let settings = Settings {
        background_image: Some(imported.to_string_lossy().into()),
        background_blur: 12.,
        background_saturation: 0.,
        ..Default::default()
    };
    let rendered = background::prepare(&settings, &root)?.expect("rendered background");
    let modified = std::fs::metadata(&rendered)?.modified()?;
    assert_eq!(
        background::prepare(&settings, &root)?.as_ref(),
        Some(&rendered)
    );
    assert_eq!(std::fs::metadata(&rendered)?.modified()?, modified);
    let rendered = image::open(rendered)?.to_rgba8();
    assert_eq!(rendered.get_pixel(10, 10)[0], rendered.get_pixel(10, 10)[1]);
    assert_eq!(std::fs::read(&original)?, bytes);
    assert_eq!(std::fs::read(&imported)?, bytes);
    // These are only the uniquely allocated temporary test fixtures.
    std::fs::remove_file(original)?;
    for entry in std::fs::read_dir(root.join("backgrounds"))? {
        std::fs::remove_file(entry?.path())?;
    }
    std::fs::remove_dir(root.join("backgrounds"))?;
    std::fs::remove_dir(root)?;
    Ok(())
}
