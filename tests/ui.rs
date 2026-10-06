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
fn keyboard_creation_typing_and_reopening_tabs(cx: &mut TestAppContext) {
    let path = std::env::temp_dir().join(format!("still-ui-{}.sqlite", uuid::Uuid::new_v4()));
    let (store, _, _, _) = Store::start(path.clone()).expect("test database");
    cx.update(gpui_kit::init);
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
