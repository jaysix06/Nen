use gpui_kit::{AssetSource, assets::IconName};
use still::assets::StillAssets;

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
        let data = StillAssets.load(&path).expect("local asset").expect("icon");
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
        let data = StillAssets.load(path).expect("window icon").expect("icon");
        assert_eq!(
            data.as_ref(),
            expected,
            "{path} must use its intended weight"
        );
    }
}
