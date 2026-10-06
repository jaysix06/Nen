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
    }
}

fn run() -> anyhow::Result<()> {
    let directory = diagnostics::data_directory()?;
    diagnostics::init(&directory)?;
    if std::env::args().any(|arg| arg == "--demo") {
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
                bounds.origin = point(px(x), px(y));
            }
            let state = cx.new(|cx| AppState::new(store, settings, session, cx));
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
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
                    let _ = handle.update(cx, |_, window, _| window.activate_window());
                    #[cfg(feature = "ui-testing")]
                    if let Some(path) = std::env::args().skip_while(|arg| arg != "--capture").nth(1)
                    {
                        let capture_state = state.clone();
                        cx.spawn(async move |cx| {
                            cx.background_executor()
                                .timer(std::time::Duration::from_millis(600))
                                .await;
                            let result = handle.update(cx, |_, window, _| {
                                window
                                    .render_to_image()
                                    .and_then(|image| image.save(path).map_err(Into::into))
                            });
                            if let Ok(Err(error)) = result {
                                eprintln!("Capture failed: {error}");
                            }
                            let _ = capture_state.update(cx, |state, cx| state.quit(cx));
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
            cx.activate(true);
            cx.spawn(async move |cx| {
                while let Ok(event) = events.recv().await {
                    let _ = state.update(cx, |state, cx| match event {
                        StoreEvent::ReminderDue(_) => {
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
