#![cfg(feature = "ui-testing")]
extern crate gpui_kit as gpui;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, TestAppContext, WindowOptions};
use still::{
    app::AppState,
    models::{Session, Settings},
    storage::{Database, Request, Store},
    ui::app_window::AppWindow,
};

#[gpui_kit::test]
fn legacy_appearance_upgrade_preserves_chosen_preferences(cx: &mut TestAppContext) {
    let path =
        std::env::temp_dir().join(format!("still-preferences-{}.sqlite", uuid::Uuid::new_v4()));
    let (store, _, _, _) = Store::start(path.clone()).expect("database");
    cx.update(gpui_kit::init);
    for (theme, color, expected_theme, wallpaper) in [
        ("System", "#f6f5f1", "Dark", true),
        ("Light", "#334455", "Light", false),
    ] {
        let settings: Settings = serde_json::from_value(serde_json::json!({"theme":theme,"surface":"Opaque","background_color":color,"editor_font_size":17.,"restore_tabs":false})).expect("legacy preferences");
        assert_eq!(settings.design_revision, 0);
        let state = cx.new(|cx| AppState::new(store.clone(), settings, Session::default(), cx));
        state.read_with(cx, |state, _| {
            assert_eq!(state.settings.theme, expected_theme);
            assert_eq!(state.settings.default_wallpaper, wallpaper);
            assert!(!state.settings.restore_tabs);
            if !wallpaper {
                assert_eq!(state.settings.background_color, color);
                assert_eq!(state.settings.editor_font_size, 17.);
            }
        });
    }
    store
        .request(Request::Shutdown)
        .recv_blocking()
        .expect("shutdown")
        .expect("flush");
    std::fs::remove_file(path).expect("cleanup");
}

#[gpui_kit::test]
fn custom_wallpapers_keep_editor_text_readable_in_both_themes(cx: &mut TestAppContext) {
    fn luminance(color: gpui_kit::Rgba) -> f32 {
        let linear = |value: f32| {
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b)
    }
    cx.update(gpui_kit::init);
    for theme in ["Dark", "Light"] {
        for surface in ["Opaque", "Frosted", "Clear"] {
            let settings = Settings {
                theme: theme.into(),
                surface: surface.into(),
                background_image: Some("white-test-wallpaper.png".into()),
                background_dim: 0.,
                ..Settings::default()
            };
            cx.update(|cx| {
                still::theme::apply(&settings, cx);
                let palette = still::theme::surfaces(&settings, cx);
                let paper = gpui_kit::Rgba::from(palette.paper);
                // White is the worst backdrop for the dark theme; black for light.
                let source = if theme == "Dark" {
                    1. - still::theme::background_dim(&settings, cx)
                } else {
                    0.
                };
                let background = gpui_kit::Rgba {
                    r: paper.r * paper.a + source * (1. - paper.a),
                    g: paper.g * paper.a + source * (1. - paper.a),
                    b: paper.b * paper.a + source * (1. - paper.a),
                    a: 1.,
                };
                for text in [palette.text, palette.muted] {
                    let a = luminance(background);
                    let b = luminance(text.into());
                    let contrast = (a.max(b) + 0.05) / (a.min(b) + 0.05);
                    assert!(contrast >= 4.5, "{theme}/{surface}: contrast {contrast}");
                }
            });
        }
    }
}

#[gpui_kit::test]
fn category_creation_moves_and_sidebar_visibility_survive_restart(cx: &mut TestAppContext) {
    cx.dispatcher.allow_parking();
    let path = std::env::temp_dir().join(format!("still-category-{}.sqlite", uuid::Uuid::new_v4()));
    let (store, mut settings, session, _) = Store::start(path.clone()).expect("database");
    // Exercise the complete keyboard/dialog flow with accessibility motion off;
    // normal-motion geometry is inspected in the native GPU captures.
    settings.reduced_motion = true;
    cx.update(gpui_kit::init);
    cx.update(|cx| still::theme::apply(&settings, cx));
    let state = cx.new(|cx| AppState::new(store.clone(), settings, session, cx));
    let (window, _) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| AppWindow::new(state.clone(), window, cx))
        })
        .expect("window")
    });
    store
        .request(Request::Categories)
        .recv_blocking()
        .expect("initial category barrier")
        .expect("initial categories");
    cx.run_until_parked();
    assert_eq!(state.read_with(cx, |state, _| state.categories.len()), 3);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("category-picker", cx);
        window.within("popup-menu").click(10usize, cx);
    })
    .expect("new category dialog");
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.input("Studio", cx);
        window.click("ok", cx);
    })
    .expect("create category");
    store
        .request(Request::Categories)
        .recv_blocking()
        .expect("category write barrier")
        .expect("categories");
    cx.run_until_parked();
    store
        .request(Request::Categories)
        .recv_blocking()
        .expect("creation refresh barrier")
        .expect("creation refresh");
    cx.run_until_parked();
    let category = state
        .read_with(cx, |state, _| state.session.category_id.clone())
        .expect("selected category");
    assert_eq!(
        state.read_with(cx, |state, _| state.category_label().to_owned()),
        "Studio"
    );
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("category-picker", cx);
        window.within("popup-menu").click(12usize, cx);
    })
    .expect("rename dialog");
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.press("ctrl-a", cx);
        window.input("Writing", cx);
        window.click("ok", cx);
    })
    .expect("rename category");
    store
        .request(Request::Categories)
        .recv_blocking()
        .expect("rename barrier")
        .expect("rename");
    cx.run_until_parked();
    store
        .request(Request::Categories)
        .recv_blocking()
        .expect("refresh barrier")
        .expect("refresh");
    cx.run_until_parked();
    assert_eq!(
        state.read_with(cx, |state, _| state.category_label().to_owned()),
        "Writing"
    );
    cx.update_window(window, |_, window, cx| window.render_frame(cx))
        .expect("frame");
    cx.simulate_keystrokes(window, "ctrl-n");
    cx.run_until_parked();
    let id = state
        .read_with(cx, |state, _| state.session.active.clone())
        .expect("note");
    assert_eq!(
        state.read_with(cx, |state, _| state.notes[&id].note.category_id.clone()),
        Some(category)
    );
    cx.simulate_keystrokes(window, "ctrl-b");
    cx.run_until_parked();
    assert!(state.read_with(cx, |state, _| state.session.sidebar_hidden));
    state.update(cx, |state, cx| {
        state.edit(
            &id,
            "Studio draft".into(),
            "Keep the latest text.".into(),
            cx,
        );
        state.move_to_category(&id, Some("work".into()), cx);
        state.select_category("work", cx);
    });
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("category-picker", cx);
        window.within("popup-menu").click(13usize, cx);
    })
    .expect("delete category dialog");
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("cancel", cx);
    })
    .expect("cancel category deletion");
    cx.run_until_parked();
    assert!(state.read_with(cx, |state, _| {
        state
            .categories
            .iter()
            .any(|category| category.id == "work")
    }));
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("category-picker", cx);
        window.within("popup-menu").click(13usize, cx);
    })
    .expect("reopen deletion dialog");
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("ok", cx);
    })
    .expect("confirm category deletion");
    store
        .request(Request::Categories)
        .recv_blocking()
        .expect("delete barrier")
        .expect("delete");
    cx.run_until_parked();
    assert_eq!(
        state.read_with(cx, |state, _| state.notes[&id].note.category_id.clone()),
        None
    );
    state.update(cx, |state, cx| state.quit(cx));
    // The real SQLite worker wakes GPUI asynchronously. Drain that handoff
    // until graceful shutdown releases its handles, rather than assuming one
    // parked executor turn means the operating-system thread has finished.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        cx.run_until_parked();
        if !matches!(
            store.request(Request::Categories).recv_blocking(),
            Ok(Ok(_))
        ) {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "shutdown timed out");
        std::thread::yield_now();
    }
    assert!(state.read_with(cx, |state, _| state.error.is_none()));
    let db = Database::open(&path).expect("reopen");
    let note = db.note(&id).expect("load").expect("note survives");
    assert_eq!(note.content, "Keep the latest text.");
    assert_eq!(note.category_id, None);
    assert!(
        db.get_setting::<Session>("session")
            .expect("session")
            .sidebar_hidden
    );
    drop(db);
    std::fs::remove_file(path).expect("cleanup");
}

#[gpui_kit::test]
fn failed_autosave_preserves_the_unsaved_buffer(cx: &mut TestAppContext) {
    cx.dispatcher.allow_parking();
    let path = std::env::temp_dir().join(format!("still-failure-{}.sqlite", uuid::Uuid::new_v4()));
    let (store, _, _, _) = Store::start(path.clone()).expect("database");
    cx.update(gpui_kit::init);
    let state =
        cx.new(|cx| AppState::new(store.clone(), Settings::default(), Session::default(), cx));
    let id = state.update(cx, |state, cx| state.create_note(false, cx));
    store
        .request(Request::Shutdown)
        .recv_blocking()
        .expect("shutdown")
        .expect("flush");
    state.update(cx, |state, cx| {
        state.edit(
            &id,
            "Unsaved title".into(),
            "Keep this text through a failure.".into(),
            cx,
        );
        state.save_now(&id, cx);
    });
    cx.run_until_parked();
    assert!(state.read_with(cx, |state, _| state.notes[&id].save_failed));
    assert_eq!(
        state.read_with(cx, |state, _| state.notes[&id].note.content.clone()),
        "Keep this text through a failure."
    );
    assert!(state.read_with(cx, |state, _| state.notes[&id].revision
        > state.notes[&id].saved_revision));
    assert!(state.read_with(cx, |state, _| state.error.is_some()));
    std::fs::remove_file(path).expect("cleanup");
}

#[gpui_kit::test]
fn an_inflight_note_action_cannot_recreate_a_deleted_note(cx: &mut TestAppContext) {
    cx.dispatcher.allow_parking();
    use still::{
        models::{Note, NoteType},
        storage::Response,
    };
    let path = std::env::temp_dir().join(format!("still-delete-{}.sqlite", uuid::Uuid::new_v4()));
    let note = Note::new(NoteType::Normal);
    {
        let db = Database::open(&path).expect("database");
        db.save_note(&note).expect("fixture");
    }
    let (store, _, _, _) = Store::start(path.clone()).expect("worker");
    cx.update(gpui_kit::init);
    let state =
        cx.new(|cx| AppState::new(store.clone(), Settings::default(), Session::default(), cx));
    state.update(cx, |state, cx| {
        state.note_action(&note.id, "pin", cx);
        state.delete(&note.id, cx);
    });
    cx.run_until_parked();
    assert!(matches!(
        store
            .request(Request::Load(note.id.clone()))
            .recv_blocking()
            .expect("reply")
            .expect("load"),
        Response::Note(None)
    ));
    cx.run_until_parked();
    assert!(!state.read_with(cx, |state, _| state.notes.contains_key(&note.id)));
    store
        .request(Request::Shutdown)
        .recv_blocking()
        .expect("shutdown")
        .expect("flush");
    std::fs::remove_file(path).expect("cleanup");
}

#[gpui_kit::test]
fn keyboard_creation_typing_and_reopening_tabs(cx: &mut TestAppContext) {
    cx.dispatcher.allow_parking();
    let path = std::env::temp_dir().join(format!("still-ui-{}.sqlite", uuid::Uuid::new_v4()));
    let (store, _, _, _) = Store::start(path.clone()).expect("test database");
    cx.update(gpui_kit::init);
    cx.update(|cx| {
        still::theme::apply(
            &Settings {
                theme: "Light".into(),
                ..Settings::default()
            },
            cx,
        );
        assert!(
            !gpui_kit::component::Theme::global(cx).is_dark(),
            "light theme applies"
        );
    });
    let state =
        cx.new(|cx| AppState::new(store.clone(), Settings::default(), Session::default(), cx));
    let (handle, _view) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| AppWindow::new(state.clone(), window, cx))
        })
        .expect("test window")
    });
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .expect("initial frame");
    cx.simulate_keystrokes(handle, "ctrl-n");
    cx.run_until_parked();
    let first = state
        .read_with(cx, |state, _| state.session.active.clone())
        .expect("new note");
    assert_eq!(state.read_with(cx, |state, _| state.session.tabs.len()), 1);
    cx.update_window(handle, |_, window, cx| {
        window.click("note-title", cx);
        window.input("Supplier call", cx);
    })
    .expect("type title");
    cx.run_until_parked();
    assert_eq!(
        state.read_with(cx, |state, _| state.notes[&first].note.title.clone()),
        "Supplier call"
    );
    cx.simulate_keystrokes(handle, "ctrl-n");
    cx.run_until_parked();
    assert_eq!(state.read_with(cx, |state, _| state.session.tabs.len()), 2);
    cx.simulate_keystrokes(handle, "ctrl-w");
    cx.run_until_parked();
    assert_eq!(state.read_with(cx, |state, _| state.session.tabs.len()), 1);
    cx.simulate_keystrokes(handle, "ctrl-shift-t");
    cx.run_until_parked();
    assert_eq!(state.read_with(cx, |state, _| state.session.tabs.len()), 2);
    state.update(cx, |state, cx| state.save_now(&first, cx));
    assert!(
        store
            .request(Request::Shutdown)
            .recv_blocking()
            .expect("shutdown")
            .is_ok()
    );
    let db = Database::open(&path).expect("reopen");
    assert_eq!(
        db.note(&first).expect("load").expect("note").title,
        "Supplier call"
    );
    drop(db);
    std::fs::remove_file(path).expect("remove test database");
}

#[gpui_kit::test]
fn floating_note_and_inline_reminder_share_persistent_state(cx: &mut TestAppContext) {
    cx.dispatcher.allow_parking();
    use still::storage::Response;
    use still::ui::floating::FloatingWindow;
    let path = std::env::temp_dir().join(format!("still-island-{}.sqlite", uuid::Uuid::new_v4()));
    let (store, _, _, _) = Store::start(path.clone()).expect("database");
    cx.update(gpui_kit::init);
    let state =
        cx.new(|cx| AppState::new(store.clone(), Settings::default(), Session::default(), cx));
    let (handle, _) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| FloatingWindow::new(state.clone(), window, cx))
        })
        .expect("island")
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("island-new", cx);
    })
    .expect("create");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.input("Quick thought", cx);
    })
    .expect("edit");
    cx.run_until_parked();
    let id = state
        .read_with(cx, |state, _| state.floating_note.clone())
        .expect("active note");
    assert_eq!(
        state.read_with(cx, |state, _| state.notes[&id].note.title.clone()),
        "Quick thought"
    );
    let mut visual = gpui_kit::VisualTestContext::from_window(handle, cx);
    visual.deactivate_window();
    cx.update_window(handle, |_, window, _| window.activate_window())
        .expect("reactivate");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.input("Keep writing after returning.", cx);
    })
    .expect("write after activation");
    cx.run_until_parked();
    assert_eq!(
        state.read_with(cx, |state, _| state.notes[&id].note.content.clone()),
        "Keep writing after returning."
    );
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("island-reminder", cx);
        window.render_frame(cx);
        window.click("save-reminder", cx);
    })
    .expect("set reminder");
    cx.run_until_parked();
    assert!(!state.read_with(cx, |state, _| state.floating_reminder));
    let response = store
        .request(Request::Reminders)
        .recv_blocking()
        .expect("read reminders")
        .expect("query");
    let Response::Reminders(reminders) = response else {
        panic!("reminder response")
    };
    assert_eq!(reminders.len(), 1);
    assert_eq!(reminders[0].note_id, id);
    assert!(
        store
            .request(Request::Shutdown)
            .recv_blocking()
            .expect("shutdown")
            .is_ok()
    );
    std::fs::remove_file(path).expect("remove database");
}

#[gpui_kit::test]
fn shortcut_capture_detects_conflicts_and_updates_commands(cx: &mut TestAppContext) {
    cx.dispatcher.allow_parking();
    use still::ui::settings::SettingsView;
    let path =
        std::env::temp_dir().join(format!("still-shortcuts-{}.sqlite", uuid::Uuid::new_v4()));
    let (store, _, _, _) = Store::start(path.clone()).expect("database");
    cx.update(gpui_kit::init);
    let state =
        cx.new(|cx| AppState::new(store.clone(), Settings::default(), Session::default(), cx));
    state.update(cx, |state, cx| {
        state.settings_page = Some("Shortcuts".into());
        state.shortcut_capture = Some("new_note".into());
        cx.notify();
    });
    let (handle, _) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| SettingsView::new(state.clone(), window, cx))
        })
        .expect("settings")
    });
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .expect("frame");
    cx.simulate_keystrokes(handle, "ctrl-w");
    cx.run_until_parked();
    assert!(state.read_with(cx, |state, _| {
        state
            .error
            .as_ref()
            .is_some_and(|error| error.contains("Close tab"))
    }));
    assert_eq!(
        state.read_with(cx, |state, _| state.settings.shortcuts["new_note"].clone()),
        "ctrl-n"
    );
    cx.simulate_keystrokes(handle, "ctrl-alt-shift-n");
    cx.run_until_parked();
    assert_eq!(
        state.read_with(cx, |state, _| state.settings.shortcuts["new_note"].clone()),
        "ctrl-alt-shift-n"
    );
    assert!(state.read_with(cx, |state, _| state.shortcut_capture.is_none()));
    assert!(
        store
            .request(Request::Shutdown)
            .recv_blocking()
            .expect("shutdown")
            .is_ok()
    );
    std::fs::remove_file(path).expect("remove database");
}
