use still::{background, models::Settings};

#[test]
fn importing_and_transforming_a_background_never_changes_the_original() -> anyhow::Result<()> {
    let root = std::env::temp_dir().join(format!("still-image-{}", uuid::Uuid::new_v4()));
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
