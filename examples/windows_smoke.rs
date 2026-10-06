//! Exercises real Windows registrations and submits a native notification.
use still::{models::*, platform::Desktop};
fn main() -> anyhow::Result<()> {
    let mut settings = Settings::default();
    for (action, key) in [
        ("toggle_float", "ctrl-alt-shift-f20"),
        ("quick_note", "ctrl-alt-shift-f21"),
        ("open_app", "ctrl-alt-shift-f22"),
    ] {
        settings.shortcuts.insert(action.into(), key.into());
    }
    let (mut desktop, events, warnings) = Desktop::new(&settings)?;
    anyhow::ensure!(warnings.is_empty(), "{warnings:?}");
    anyhow::ensure!(desktop.tray.is_some(), "Windows tray did not initialize");
    let reminder = Reminder {
        id: uuid::Uuid::new_v4().to_string(),
        note_id: uuid::Uuid::new_v4().to_string(),
        title: "Still test reminder".into(),
        preview: "Native notification delivery check.".into(),
        scheduled_at: chrono::Utc::now().timestamp(),
        recurrence: Recurrence::Never,
        status: "pending".into(),
        series_id: None,
    };
    desktop.notify(&reminder, false)?;
    std::thread::sleep(std::time::Duration::from_secs(2));
    while let Ok(event) = events.try_recv() {
        anyhow::ensure!(
            !matches!(event, still::platform::PlatformEvent::Error(_)),
            "{event:?}"
        );
    }
    println!("Native Windows hotkeys, tray and notification submission succeeded.");
    Ok(())
}
