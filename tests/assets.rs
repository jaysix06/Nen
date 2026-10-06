use gpui_kit::{AssetSource, assets::IconName};
use still::assets::StillAssets;

#[test]
fn application_icons_use_the_embedded_phosphor_fill_artwork() {
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
    for path in [
        "icons/window-minimize.svg",
        "icons/window-maximize.svg",
        "icons/window-restore.svg",
        "icons/window-close.svg",
    ] {
        let data = StillAssets.load(path).expect("window icon").expect("icon");
        assert!(
            std::str::from_utf8(&data)
                .expect("SVG")
                .contains("0 0 256 256")
        );
    }
}
