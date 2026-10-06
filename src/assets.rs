use gpui_kit::{AssetSource, SharedString};
use std::borrow::Cow;

/// Local Phosphor Fill artwork, including icons drawn by GPUI's controls.
pub struct StillAssets;

impl AssetSource for StillAssets {
    fn load(&self, path: &str) -> gpui_kit::Result<Option<Cow<'static, [u8]>>> {
        macro_rules! icon {
            ($name:literal) => {
                Some(Cow::Borrowed(
                    include_bytes!(concat!("../assets/phosphor-fill/", $name, "-fill.svg"))
                        .as_slice(),
                ))
            };
        }
        let data = match path {
            "icons/bell.svg" => icon!("bell"),
            "icons/pin.svg" => icon!("push-pin"),
            "icons/plus.svg" => icon!("plus"),
            "icons/search.svg" => icon!("magnifying-glass"),
            "icons/settings.svg" => icon!("gear"),
            "icons/x.svg" | "icons/window-close.svg" => icon!("x"),
            "icons/panel-left.svg" => icon!("sidebar-simple"),
            "icons/chevron-down.svg" => icon!("caret-down"),
            "icons/chevron-up.svg" => icon!("caret-up"),
            "icons/chevron-right.svg" => icon!("caret-right"),
            "icons/chevron-left.svg" => icon!("caret-left"),
            "icons/book-open.svg" => icon!("book-open"),
            "icons/pencil.svg" => icon!("pencil-simple"),
            "icons/ellipsis.svg" => icon!("dots-three"),
            "icons/arrow-left.svg" => icon!("arrow-left"),
            "icons/arrow-up-right.svg" => icon!("arrow-up-right"),
            "icons/notebook-pen.svg" => icon!("note-pencil"),
            "icons/check.svg" => icon!("check"),
            "icons/clock.svg" => icon!("clock"),
            "icons/rotate-ccw.svg" => icon!("arrow-counter-clockwise"),
            "icons/palette.svg" => icon!("palette"),
            "icons/image.svg" => icon!("image"),
            "icons/app-window.svg" => icon!("app-window"),
            "icons/keyboard.svg" => icon!("keyboard"),
            "icons/sliders-horizontal.svg" => icon!("sliders-horizontal"),
            "icons/window-minimize.svg" | "icons/minus.svg" => icon!("minus"),
            "icons/window-maximize.svg" => icon!("square"),
            "icons/window-restore.svg" => icon!("copy-simple"),
            "icons/eye.svg" => icon!("eye"),
            "icons/eye-off.svg" => icon!("eye-slash"),
            "icons/loader.svg" | "icons/loader-circle.svg" => icon!("circle-notch"),
            "icons/circle-alert.svg" | "icons/info.svg" => icon!("info"),
            "icons/circle-check.svg" => icon!("check-circle"),
            "icons/calendar.svg" => icon!("calendar-blank"),
            "icons/trash.svg" | "icons/trash-2.svg" => icon!("trash"),
            "icons/copy.svg" => icon!("copy"),
            "icons/archive.svg" => icon!("archive"),
            "icons/image-off.svg" => icon!("image-broken"),
            _ => return gpui_kit::assets::AllAssets.load(path),
        };
        Ok(data)
    }

    fn list(&self, path: &str) -> gpui_kit::Result<Vec<SharedString>> {
        gpui_kit::assets::AllAssets.list(path)
    }
}
