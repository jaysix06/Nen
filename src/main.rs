#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use gpui_kit::*;
use nen::{
    app::AppState,
    diagnostics,
    models::*,
    storage::{Database, Store, StoreEvent},
    theme,
    ui::app_window::AppWindow,
};

fn main() {
    if std::env::args().nth(1).as_deref() == Some("--apply-update") {
        let result = std::env::args()
            .nth(2)
            .ok_or_else(|| anyhow::anyhow!("Missing update parent"))
            .and_then(|pid| Ok(pid.parse::<u32>()?))
            .and_then(nen::update::apply_update);
        if let Err(error) = result {
            eprintln!("Nen update failed: {error}");
            std::process::exit(1);
        }
        return;
    }
    if let Err(error) = run() {
        eprintln!("Could not start Nen: {error}");
        log::error!("Startup failed: {error}");
        nen::platform::startup_error(&error.to_string());
    }
}

fn run() -> anyhow::Result<()> {
    let directory = diagnostics::data_directory()?;
    diagnostics::init(&directory)?;
    if let Some(path) = std::env::args_os()
        .skip_while(|arg| arg != "--cleanup-update")
        .nth(1)
    {
        std::thread::spawn(move || {
            if let Err(error) = nen::update::cleanup_update(std::path::Path::new(&path)) {
                log::warn!("Update cleanup failed: {error}");
            }
        });
    }
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    directory.hash(&mut hash);
    // Share the legacy mutex so an older running instance cannot race the same database.
    let Some(_instance) = nen::platform::instance(&format!("Local\\Still-{:x}", hash.finish()))?
    else {
        return Ok(());
    };
    if std::env::args().any(|arg| arg == "--demo") {
        anyhow::ensure!(
            std::env::var_os("NEN_DATA_DIR").is_some()
                || std::env::var_os("STILL_DATA_DIR").is_some(),
            "Use NEN_DATA_DIR for demo data so your notes stay separate."
        );
        seed_demo(&directory.join("notes.sqlite"))?;
    }
    let (store, settings, session, events) = Store::start(directory.join("notes.sqlite"))?;
    gpui_kit::application()
        .with_assets(nen::assets::NenAssets)
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
                    title: Some("Nen".into()),
                    appears_transparent: true,
                    ..Default::default()
                }),
                app_owns_titlebar_drag: true,
                window_min_size: Some(size(px(820.), px(540.))),
                app_id: Some(nen::platform::APP_ID.into()),
                ..Default::default()
            };
            match gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| AppWindow::new(state.clone(), window, cx))
            }) {
                Ok((handle, _main_view)) => {
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
                        let capture_view = _main_view.clone();
                        cx.spawn(async move |cx| {
                            cx.background_executor()
                                .timer(std::time::Duration::from_millis(100))
                                .await;
                            let floating = std::env::args().any(|arg| arg == "--capture-floating");
                            capture_state.update(cx, |state, cx| {
                                let mut settings = state.settings.clone();
                                settings.theme =
                                    if std::env::args().any(|arg| arg == "--capture-light") {
                                        "Light"
                                    } else {
                                        "Dark"
                                    }
                                    .into();
                                if let Some(font) = std::env::args()
                                    .skip_while(|arg| arg != "--capture-font")
                                    .nth(1)
                                    .and_then(|value| value.parse::<f32>().ok())
                                {
                                    settings.editor_font_size = font.clamp(12., 30.);
                                }
                                state.update_settings(settings, cx);
                                if std::env::args().any(|arg| arg == "--capture-update-banner") {
                                    // Offline preview fixture, available only in UI-testing builds.
                                    state.update = Some(nen::update::Update {
                                        version: "0.2.9".into(),
                                        download_url: "https://invalid.example/Nen.exe".into(),
                                        size: 128,
                                        sha256: "a".repeat(64),
                                    });
                                }
                                if std::env::args().any(|arg| arg == "--capture-settings") {
                                    state.settings_page = Some("Appearance".into());
                                }
                                if std::env::args().any(|arg| arg == "--capture-background") {
                                    state.settings_page = Some("Background".into());
                                }
                                if std::env::args().any(|arg| arg == "--capture-work") {
                                    let note = state
                                        .summaries
                                        .iter()
                                        .find(|note| note.category_id.as_deref() == Some("work"))
                                        .map(|note| note.id.clone());
                                    state.select_category("work", cx);
                                    if let Some(id) = note {
                                        state.open_note(&id, cx);
                                    }
                                }
                                if std::env::args().any(|arg| arg == "--capture-sidebar-hidden") {
                                    state.session.sidebar_hidden = true;
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
                                    nen::ui::floating::show(state.clone(), true, cx);
                                    if std::env::args().any(|arg| arg == "--capture-results")
                                        && let Some(view) = state.read(cx).floating_view.clone()
                                    {
                                        view.update(cx, |view, cx| view.expand_for_capture(cx));
                                    }
                                });
                            }
                            // Capture the settled image layer as well as the native UI.
                            // This bounded wait exists only in the UI-testing build.
                            for _ in 0..100 {
                                let pending = capture_state.read_with(cx, |state, _| {
                                    (state.settings.default_wallpaper
                                        || state.settings.background_image.is_some())
                                        && state.background.is_none()
                                });
                                if !pending {
                                    break;
                                }
                                cx.background_executor()
                                    .timer(std::time::Duration::from_millis(100))
                                    .await;
                            }
                            cx.background_executor()
                                .timer(std::time::Duration::from_millis(800))
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
                            if std::env::args().any(|arg| arg == "--check-note-content") {
                                let (marker, image_offset) = capture_state.read_with(cx, |state, _| {
                                    let id = state.session.active.as_ref().expect("active note");
                                    let text = &state.notes[id].note.content;
                                    (nen::ui::lists::checklist_items(text)[0].marker.start, text.find("![").expect("fixture image"))
                                });
                                target.update(cx, |_, window, cx| {
                                    use gpui_kit::test::TestWindowExt;
                                    let box_id = SharedString::from(format!("read-check-{marker}"));
                                    window.click(box_id, cx);
                                }).expect("native checkbox click");
                                cx.background_executor().timer(std::time::Duration::from_millis(150)).await;
                                target.update(cx, |_, window, cx| {
                                    use gpui_kit::test::TestWindowExt;
                                    let box_id = SharedString::from(format!("read-check-{marker}"));
                                    window.render_frame(cx);
                                    assert_eq!(window.find(box_id.clone()).checked(), Some(true));
                                    window.press("space", cx);
                                }).expect("native checkbox keyboard activation");
                                cx.background_executor().timer(std::time::Duration::from_millis(150)).await;
                                target.update(cx, |_, window, cx| {
                                    use gpui_kit::test::TestWindowExt;
                                    let box_id = SharedString::from(format!("read-check-{marker}"));
                                    window.render_frame(cx);
                                    assert_eq!(window.find(box_id).checked(), Some(false));
                                    let image_id = SharedString::from(format!("note-image-{image_offset}"));
                                    let original = window.find(image_id.clone()).bounds().size;
                                    let body = window.find("note-body").bounds();
                                    for delta in [1., -1.] {
                                        window.dispatch_event(ScrollWheelEvent { position: body.center(), delta: ScrollDelta::Lines(point(0., delta)), modifiers: Modifiers { control: true, ..Default::default() }, ..Default::default() }.to_platform_input(), cx);
                                        window.render_frame(cx);
                                        let image = window.find(image_id.clone()).bounds().size;
                                        if delta > 0. { assert!(image.width > original.width && image.height > original.height); }
                                        else { assert_eq!(image, original); }
                                    }
                                    assert_eq!(window.find("note-footer").bounds().top(), window.find("notes-footer").bounds().top());
                                }).expect("native checklist and zoom check");
                                eprintln!("Native checkbox click, keyboard toggle, image zoom and footer alignment verified");
                            }
                            if std::env::args().any(|arg| arg == "--check-image-paste") {
                                let id = capture_state.read_with(cx, |state, _| state.session.active.clone().expect("active fixture note"));
                                let before = capture_state.read_with(cx, |state, _| state.notes[&id].note.content.matches("nen-image://").count());
                                target.update(cx, |_, window, cx| {
                                    let editor = capture_view.read(cx).editors[&id].clone();
                                    editor.update(cx, |view, cx| view.focus_body(window, cx));
                                    use gpui_kit::test::TestWindowExt;
                                    window.render_frame(cx);
                                    window.press("ctrl-v", cx);
                                }).expect("native image paste");
                                for _ in 0..100 {
                                    if capture_state.read_with(cx, |state, _| state.notes[&id].note.content.matches("nen-image://").count() > before) { break; }
                                    cx.background_executor().timer(std::time::Duration::from_millis(50)).await;
                                }
                                assert_eq!(capture_state.read_with(cx, |state, _| state.notes[&id].note.content.matches("nen-image://").count()), before + 1, "native clipboard image inserted");
                                target.update(cx, |_, window, cx| {
                                    use gpui_kit::test::TestWindowExt;
                                    window.render_frame(cx);
                                    if std::env::args().any(|arg| arg == "--capture-reading") { window.click("read-note", cx); }
                                }).expect("return to reading view");
                                capture_state.update(cx, |state, cx| { state.message = None; cx.notify(); });
                                cx.background_executor().timer(std::time::Duration::from_millis(500)).await;
                                eprintln!("Native Windows image paste verified");
                            }
                            if std::env::args().any(|arg| arg == "--check-category-shortcuts") {
                                let expected = [
                                    "All notes",
                                    "Pinned",
                                    "Personal",
                                    "Work",
                                    "Ideas",
                                    "Reminders",
                                    "Archive",
                                    "Archive",
                                    "Archive",
                                ];
                                for (index, expected) in expected.iter().enumerate() {
                                    target
                                        .update(cx, |_, window, cx| {
                                            // Feed the event representation produced by the actual
                                            // Windows keyboard mapper, including shifted digit symbols.
                                            let key = gpui_kit::Keystroke::parse(&format!(
                                                "ctrl-shift-{}",
                                                index + 1
                                            ))
                                            .expect("test shortcut");
                                            let event = cx
                                                .keyboard_mapper()
                                                .map_key_equivalent(key, false)
                                                .inner()
                                                .clone();
                                            window.dispatch_keystroke(event, cx);
                                        })
                                        .expect("native keyboard check");
                                    assert_eq!(
                                        capture_state.read_with(cx, |state, _| state
                                            .category_label()
                                            .to_owned()),
                                        *expected
                                    );
                                }
                                target
                                    .update(cx, |_, window, cx| {
                                        let key = gpui_kit::Keystroke::parse("ctrl-shift-4")
                                            .expect("test shortcut");
                                        let event = cx
                                            .keyboard_mapper()
                                            .map_key_equivalent(key, false)
                                            .inner()
                                            .clone();
                                        window.dispatch_keystroke(event, cx);
                                    })
                                    .expect("restore Work category");
                                eprintln!("Native numeric category shortcuts verified");
                            }
                            if std::env::args().any(|arg| {
                                arg == "--capture-categories" || arg == "--capture-category-form"
                            }) {
                                let _ = target.update(cx, |_, window, cx| {
                                    use gpui_kit::test::TestWindowExt;
                                    window.click("category-picker", cx);
                                    window.render_frame(cx);
                                    if std::env::args().any(|arg| arg == "--capture-category-form")
                                    {
                                        window.within("popup-menu").click(10usize, cx);
                                        window.render_frame(cx);
                                    }
                                });
                                cx.background_executor()
                                    .timer(std::time::Duration::from_millis(250))
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
            state.update(cx, |state, cx| {
                if std::env::args().any(|arg| arg == "--update-failed") {
                    state.fail("Nen couldn't install the update. Your previous version has been reopened. Try again from the update banner.".into(), cx);
                }
                if !std::env::args().any(|arg| arg == "--demo" || arg == "--capture") {
                    state.check_for_updates(cx);
                }
            });
            let config = state.read(cx).settings.clone();
            match nen::platform::Desktop::new(&config) {
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
                                use nen::platform::PlatformEvent;
                                match event {
                                    PlatformEvent::Wake => {
                                        let _ = state.store.request(nen::storage::Request::Wake);
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
                                                    nen::storage::Request::Complete(id.into()),
                                                    "Reminder completed",
                                                    cx,
                                                ),
                                                "snooze" => state.perform(
                                                    nen::storage::Request::Snooze(
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
                let _ = handle.update(cx, |_, window, cx| nen::platform::hide(window, cx));
            }
            if config.floating_on_startup {
                nen::ui::floating::show(state.clone(), true, cx);
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
        note.category_id = Some(
            if title.starts_with("Monday") {
                "work"
            } else if title.starts_with("Ideas") {
                "ideas"
            } else {
                "personal"
            }
            .into(),
        );
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
