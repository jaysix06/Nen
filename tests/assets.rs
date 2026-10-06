use gpui_kit::{AssetSource, assets::IconName};
use nen::assets::NenAssets;

#[test]
fn application_icon_preserves_transparency_and_windows_sizes() {
    let ico = include_bytes!("../assets/nen.ico");
    assert_eq!(&ico[..6], &[0, 0, 1, 0, 7, 0]);
    for (index, size) in [16u32, 24, 32, 48, 64, 128, 256].into_iter().enumerate() {
        let entry = &ico[6 + index * 16..6 + (index + 1) * 16];
        let length = u32::from_le_bytes(entry[8..12].try_into().expect("frame length")) as usize;
        let offset = u32::from_le_bytes(entry[12..16].try_into().expect("frame offset")) as usize;
        let frame = &ico[offset..offset + length];
        let image = image::load_from_memory(frame)
            .expect("PNG icon frame")
            .into_rgba8();
        assert_eq!(image.dimensions(), (size, size));
        assert_eq!(image.get_pixel(0, 0).0[3], 0, "transparent corners");
        assert!(
            image.get_pixel(size / 2, size / 2).0[3] > 0,
            "visible artwork"
        );
        if size == 32 {
            assert_eq!(frame, include_bytes!("../assets/nen-tray.png"));
        }
        if size == 256 {
            assert_eq!(frame, include_bytes!("../assets/nen.png"));
        }
    }
}

#[test]
fn application_icons_use_the_embedded_phosphor_artwork() {
    for icon in [
        IconName::Bell,
        IconName::Pin,
        IconName::Plus,
        IconName::Search,
        IconName::Settings,
        IconName::X,
        IconName::PanelLeft,
        IconName::ChevronDown,
        IconName::BookOpen,
        IconName::Pencil,
        IconName::Ellipsis,
        IconName::ArrowLeft,
        IconName::ArrowUpRight,
        IconName::NotebookPen,
        IconName::Check,
        IconName::Clock,
        IconName::RotateCcw,
        IconName::Palette,
        IconName::Image,
        IconName::AppWindow,
        IconName::Keyboard,
        IconName::SlidersHorizontal,
    ] {
        let path = icon.path();
        let data = NenAssets.load(&path).expect("local asset").expect("icon");
        let svg = std::str::from_utf8(&data).expect("SVG");
        assert!(
            svg.contains("0 0 256 256"),
            "{path} must use Phosphor artwork"
        );
    }
    for (path, expected) in [
        (
            "icons/plus.svg",
            include_bytes!("../assets/phosphor-regular/plus.svg").as_slice(),
        ),
        (
            "icons/minus.svg",
            include_bytes!("../assets/phosphor-regular/minus.svg").as_slice(),
        ),
        (
            "icons/x.svg",
            include_bytes!("../assets/phosphor-regular/x.svg").as_slice(),
        ),
        (
            "icons/window-minimize.svg",
            include_bytes!("../assets/phosphor-regular/minus.svg").as_slice(),
        ),
        (
            "icons/window-maximize.svg",
            include_bytes!("../assets/phosphor-regular/square.svg").as_slice(),
        ),
        (
            "icons/window-restore.svg",
            include_bytes!("../assets/phosphor-regular/copy-simple.svg").as_slice(),
        ),
        (
            "icons/window-close.svg",
            include_bytes!("../assets/phosphor-regular/x.svg").as_slice(),
        ),
        (
            "icons/bell.svg",
            include_bytes!("../assets/phosphor-fill/bell-fill.svg").as_slice(),
        ),
    ] {
        let data = NenAssets.load(path).expect("window icon").expect("icon");
        assert_eq!(
            data.as_ref(),
            expected,
            "{path} must use its intended weight"
        );
    }
}
