#![cfg(windows)]
use global_hotkey::GlobalHotKeyManager;
use nen::{
    models::Settings,
    platform::{Desktop, parse_hotkey},
};

#[test]
fn global_shortcut_conflict_restores_the_old_registration() -> anyhow::Result<()> {
    let mut settings = Settings::default();
    for (action, key) in [
        ("toggle_float", "ctrl-alt-shift-f20"),
        ("quick_note", "ctrl-alt-shift-f21"),
        ("open_app", "ctrl-alt-shift-f22"),
    ] {
        settings.shortcuts.insert(action.into(), key.into());
    }
    let (mut desktop, _, warnings) = Desktop::new(&settings)?;
    assert!(warnings.is_empty(), "{warnings:?}");
    let other = GlobalHotKeyManager::new()?;
    let occupied = parse_hotkey("ctrl-alt-shift-f23")?;
    other.register(occupied)?;
    assert!(
        desktop
            .change_hotkey("toggle_float", "ctrl-alt-shift-f23")
            .is_err()
    );
    let original = parse_hotkey("ctrl-alt-shift-f20")?;
    assert!(
        other.register(original).is_err(),
        "the original shortcut must be registered again"
    );
    other.unregister(occupied)?;
    desktop.change_hotkey("toggle_float", "ctrl-alt-shift-f23")?;
    other.register(original)?;
    other.unregister(original)?;
    desktop.replace_hotkeys(&settings.shortcuts)?;
    assert_eq!(
        desktop.action_for_hotkey(original.id()),
        Some("toggle_float")
    );
    Ok(())
}

#[test]
fn modifier_and_punctuation_shortcuts_are_supported() -> anyhow::Result<()> {
    parse_hotkey("ctrl-alt-shift-f24")?;
    parse_hotkey("ctrl--")?;
    parse_hotkey("ctrl-shift-+")?;
    parse_hotkey("⊞n")?;
    Ok(())
}
