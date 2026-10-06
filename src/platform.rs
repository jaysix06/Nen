#[cfg(windows)]
mod notification_activation;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;

/// Keep physical key modifiers when Windows reports shifted symbols such as `!`.
pub(crate) fn shortcut_string(key: &gpui_kit::Keystroke, cx: &gpui_kit::App) -> String {
    let mapped = cx.keyboard_mapper().map_key_equivalent(key.clone(), false);
    gpui_kit::Keystroke {
        key: mapped.key().to_owned(),
        modifiers: *mapped.modifiers(),
        key_char: None,
    }
    .to_string()
    .to_ascii_lowercase()
}
