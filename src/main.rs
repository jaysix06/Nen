#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use gpui_kit::*;
use still::{
    app::AppState,
    diagnostics,
    models::*,
    storage::{Database, Store, StoreEvent},
    theme,
    ui::app_window::AppWindow,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("Could not start Still: {error}");
        log::error!("Startup failed: {error}");
        still::platform::startup_error(&error.to_string());
    }
}

fn run() -> anyhow::Result<()> {
    let directory = diagnostics::data_directory()?;
    diagnostics::init(&directory)?;
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    directory.hash(&mut hash);
    let Some(_instance) = still::platform::instance(&format!("Local\\Still-{:x}", hash.finish()))?
    else {
        return Ok(());
    };
    if std::env::args().any(|arg| arg == "--demo") {
        anyhow::ensure!(
            std::env::var_os("STILL_DATA_DIR").is_some(),
            "Use STILL_DATA_DIR for demo data so your notes stay separate."
        );
        seed_demo(&directory.join("notes.sqlite"))?;
    }
    let (store, settings, session, events) = Store::start(directory.join("notes.sqlite"))?;
    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(move |cx| {
            gpui_kit::init(cx);
            theme::apply(&settings, cx);
            let width = if session.width >= 820. {
                session.width
            } else {
                1180.
            };
            let height = if session.height >= 540. {
                session.height
            } else {
                780.
            };
            let mut bounds = Bounds::centered(None, size(px(width), px(height)), cx);
            if let (Some(x), Some(y)) = (session.x, session.y) {
                let origin = point(px(x), px(y));
                if cx
                    .displays()
                    .iter()
                    .any(|display| display.bounds().contains(&point(px(x + 60.), px(y + 20.))))
                {
                    bounds.origin = origin;
                }
            }
            let state = cx.new(|cx| AppState::new(store, settings, session, cx));
            let options = WindowOptions {
                // Present only after GPUI finishes creating the view. Windows
                // can synchronously request a frame while showing a window.
                show: false,
                window_bounds: Some(if state.read(cx).session.maximized {
                    WindowBounds::Maximized(bounds)
                } else {
                    WindowBounds::Windowed(bounds)
                }),
                titlebar: Some(TitlebarOptions {
                    title: Some("Still".into()),
                    ..Default::default()
                }),
                window_min_size: Some(size(px(820.), px(540.))),
                app_id: Some("dev.still.notes".into()),
                ..Default::default()
            };
            match gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| AppWindow::new(state.clone(), window, cx))
            }) {
                Ok((handle, _)) => {
                    state.update(cx, |state, _| state.main_window = Some(handle));
                    if !(std::env::args().any(|arg| arg == "--startup")
                        && state.read(cx).settings.start_in_tray)
                    {
                        let _ = handle.update(cx, |_, window, _| window.activate_window());
                    }
                    #[cfg(feature = "ui-testing")]
                    if let Some(path) = std::env::args().skip_while(|arg| arg != "--capture").nth(1)
                    {
                        let capture_state = state.clone();
                        cx.spawn(async move |cx| {
                            cx.background_executor()
                                .timer(std::time::Duration::from_millis(100))
                                .await;
                            let floating = std::env::args().any(|arg| arg == "--capture-floating");
                            capture_state.update(cx, |state, cx| {
                                let mut settings = state.settings.clone();
                                settings.theme = "Light".into();
                                state.update_settings(settings, cx);
                                if std::env::args().any(|arg| arg == "--capture-settings") {
                                    state.settings_page = Some("Appearance".into());
                                }
                                if floating && std::env::args().any(|arg| arg == "--capture-note") {
                                    state.floating_note = state.session.active.clone();
                                }
                                if floating
                                    && std::env::args().any(|arg| arg == "--capture-reminder")
                                {
                                    state.floating_note = state.session.active.clone();
                                    state.floating_reminder = true;
                                }
                                cx.notify();
                            });
                            if floating {
                                let state = capture_state.clone();
                                cx.update(|cx| {
                                    still::ui::floating::show(state.clone(), true, cx);
                                    if std::env::args().any(|arg| arg == "--capture-results")
                                        && let Some(view) = state.read(cx).floating_view.clone()
                                    {
                                        view.update(cx, |view, cx| view.expand_for_capture(cx));
                                    }
                                });
                            }
                            cx.background_executor()
                                .timer(std::time::Duration::from_millis(600))
                                .await;
                            let target = if floating {
                                capture_state
                                    .read_with(cx, |state, _| state.floating_window)
                                    .unwrap_or(handle)
                            } else {
                                handle
                            };
                            for _ in 0..4 {
                                let _ = target.update(cx, |_, window, cx| {
                                    use gpui_kit::test::TestWindowExt;
                                    window.render_frame(cx);
                                });
                                cx.background_executor()
                                    .timer(std::time::Duration::from_millis(80))
                                    .await;
                            }
                            let result = target.update(cx, |_, window, cx| {
                                use gpui_kit::test::TestWindowExt;
                                window.render_frame(cx);
                                window
                                    .render_to_image()
                                    .and_then(|image| image.save(path).map_err(Into::into))
                            });
                            if let Ok(Err(error)) = result {
                                eprintln!("Capture failed: {error}");
                            }
                            capture_state.update(cx, |state, cx| state.quit(cx));
                        })
                        .detach();
                    }
                }
                Err(error) => {
                    log::error!("Could not open window: {error}");
                    cx.quit();
                    return;
                }
            }
            let config = state.read(cx).settings.clone();
            match still::platform::Desktop::new(&config) {
                Ok((desktop, platform_events, warnings)) => {
                    if let Some(handle) = state.read(cx).main_window {
                        let result = handle.update(cx, |_, window, _| desktop.attach(window));
                        if let Ok(Err(error)) = result {
                            state.update(cx, |state, cx| state.fail(error.to_string(), cx));
                        }
                    }
                    state.update(cx, |state, cx| {
                        state.desktop = Some(desktop);
                        for warning in warnings {
                            state.fail(warning, cx);
                        }
                    });
                    let platform_state = state.clone();
                    cx.spawn(async move |cx| {
                        while let Ok(event) = platform_events.recv().await {
                            platform_state.update(cx, |state, cx| {
                                use still::platform::PlatformEvent;
                                match event {
                                    PlatformEvent::Wake => {
                                        let _ = state.store.request(still::storage::Request::Wake);
                                    }
                                    PlatformEvent::Hotkey(id) => {
                                        let command = state
                                            .desktop
                                            .as_ref()
                                            .and_then(|d| d.action_for_hotkey(id))
                                            .map(str::to_owned);
                                        if let Some(command) = command {
                                            state.desktop_command(&command, cx);
                                        }
                                    }
                                    PlatformEvent::Command(command) => {
                                        state.desktop_command(&command, cx)
                                    }
                                    PlatformEvent::Appearance => {
                                        theme::apply(&state.settings, cx);
                                        cx.notify();
                                    }
                                    PlatformEvent::Error(error) => state.fail(error, cx),
                                    PlatformEvent::Notification(arguments) => {
                                        if let Some((action, id)) = arguments.split_once(':') {
                                            match action {
                                                "open" => {
                                                    state.open_note(id, cx);
                                                    state.desktop_command("open_app", cx);
                                                }
                                                "done" => state.perform(
                                                    still::storage::Request::Complete(id.into()),
                                                    "Reminder completed",
                                                    cx,
                                                ),
                                                "snooze" => state.perform(
                                                    still::storage::Request::Snooze(
                                                        id.into(),
                                                        state.settings.snooze_minutes,
                                                    ),
                                                    "Reminder snoozed",
                                                    cx,
                                                ),
                                                _ => {}
                                            }
                                        }
                                    }
                                }
                            });
                        }
                    })
                    .detach();
                }
                Err(error) => state.update(cx, |state, cx| {
                    state.fail(format!("Windows integration couldn't start: {error}"), cx)
                }),
            }
            theme::apply(&config, cx);
            let start_in_tray = std::env::args().any(|arg| arg == "--startup")
                && config.start_in_tray
                && state
                    .read(cx)
                    .desktop
                    .as_ref()
                    .is_some_and(|d| d.tray.is_some());
            if start_in_tray && let Some(handle) = state.read(cx).main_window {
                let _ = handle.update(cx, |_, window, cx| still::platform::hide(window, cx));
            }
            if config.floating_on_startup {
                still::ui::floating::show(state.clone(), true, cx);
            }
            if !start_in_tray {
                cx.activate(true);
            }
            cx.spawn(async move |cx| {
                while let Ok(event) = events.recv().await {
                    state.update(cx, |state, cx| match event {
                        StoreEvent::ReminderDue(reminder) => {
                            let sound = state.settings.notification_sound;
                            if let Some(desktop) = state.desktop.as_mut()
                                && let Err(error) = desktop.notify(&reminder, sound)
                            {
                                state.fail(format!("Couldn't display this reminder: {error}"), cx);
                            }
                            state.refresh_reminders(cx);
                            state.toast("A reminder is due", cx);
                        }
                        StoreEvent::Error(error) => state.fail(error, cx),
                    });
                }
            })
            .detach();
        });
    Ok(())
}

fn seed_demo(path: &std::path::Path) -> anyhow::Result<()> {
    let db = Database::open(path)?;
    if !db.list(Collection::All, "")?.is_empty() {
        return Ok(());
    }
    for (title, content, pinned) in [
        (
            "Small things, worth keeping",
            "A good thought doesn't need a system. Just somewhere to land.\n\n## This week\n\n- Take the long way home\n- Find a quiet hour for reading\n- Finish what I started\n\n## A reminder to myself\n\nMake a little space before adding something new.",
            true,
        ),
        (
            "The bookshop on the corner",
            "Ask if they have a copy of A Month in the Country.\n\nOpen until 6 on weekdays. Coffee next door.",
            true,
        ),
        (
            "Monday's conversation",
            "## A few things to follow up\n\n- [ ] Confirm the delivery date\n- [ ] Send the revised sample\n- [x] Share the meeting notes\n\nKeep the first version small. We can revisit the rest on Friday.",
            false,
        ),
        (
            "Ideas for a slower weekend",
            "A walk by the water.\nLunch somewhere new.\nNothing scheduled after three.",
            false,
        ),
        (
            "A useful little list",
            "Spare charging cable\nOlive oil\nA notebook with plain pages",
            false,
        ),
    ] {
        let mut note = Note::new(NoteType::Normal);
        note.title = title.into();
        note.content = content.into();
        note.is_pinned = pinned;
        db.save_note(&note)?;
    }
    let notes = db.list(Collection::All, "")?;
    let session = Session {
        tabs: notes.iter().take(3).map(|n| n.id.clone()).collect(),
        active: notes.first().map(|n| n.id.clone()),
        ..Default::default()
    };
    db.set_setting("session", &session)?;
    Ok(())
}
