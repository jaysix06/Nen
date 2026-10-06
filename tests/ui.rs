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
