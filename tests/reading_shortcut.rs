#![cfg(feature = "ui-testing")]
extern crate gpui_kit as gpui;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, TestAppContext, WindowOptions};
use nen::{
    app::AppState,
    models::{Session, Settings},
    storage::{Request, Store},
    ui::{app_window::AppWindow, floating::FloatingWindow},
};

#[gpui_kit::test]
fn reading_shortcut_toggles_both_windows_and_restores_typing(cx: &mut TestAppContext) {
    cx.dispatcher.allow_parking();
    cx.update(gpui_kit::init);
    for floating in [false, true] {
        let path = std::env::temp_dir().join(format!(
            "nen-reading-shortcut-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        let (store, _, _, _) = Store::start(path.clone()).expect("database");
        let mut settings = Settings {
            default_wallpaper: false,
            reduced_motion: true,
            ..Default::default()
        };
        // Existing installations receive the new default through settings migration.
        settings.shortcuts.remove("toggle_reading");
        let state = cx.new(|cx| AppState::new(store.clone(), settings, Session::default(), cx));
        assert_eq!(
            state.read_with(cx, |state, _| state.settings.shortcuts["toggle_reading"]
                .clone()),
            "ctrl-shift-r"
        );
        let handle = cx.update(|cx| {
            if floating {
                gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                    cx.new(|cx| FloatingWindow::new(state.clone(), window, cx))
                })
                .expect("floating window")
                .0
            } else {
                gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                    cx.new(|cx| AppWindow::new(state.clone(), window, cx))
                })
                .expect("main window")
                .0
            }
        });
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            if floating {
                window.click("island-new", cx);
            } else {
                state.update(cx, |state, cx| {
                    state.create_note(false, cx);
                });
            }
        })
        .expect("new note");
        cx.run_until_parked();
        let id = state.read_with(cx, |state, _| {
            if floating {
                state.floating_note.clone()
            } else {
                state.session.active.clone()
            }
            .expect("active note")
        });
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.click("note-title", cx);
        })
        .expect("title focus");
        for shortcut in ["ctrl-shift-r", "ctrl-alt-r"] {
            if shortcut == "ctrl-alt-r" {
                state.update(cx, |state, cx| {
                    nen::ui::settings::set_binding(state, "toggle_reading", shortcut)
                        .expect("custom shortcut");
                    cx.notify();
                });
                cx.update_window(handle, |_, window, cx| window.render_frame(cx))
                    .expect("binding frame");
                cx.simulate_keystrokes(handle, "ctrl-shift-r");
                cx.update_window(handle, |_, window, cx| {
                    window.render_frame(cx);
                    assert!(window.try_find("reading-view").is_none());
                })
                .expect("old binding inactive");
            }
            cx.simulate_keystrokes(handle, shortcut);
            cx.update_window(handle, |_, window, cx| {
                window.render_frame(cx);
                assert!(window.find("reading-view").visible());
            })
            .expect("reading mode");
            cx.simulate_keystrokes(handle, shortcut);
            cx.update_window(handle, |_, window, cx| {
                window.render_frame(cx);
                assert!(window.try_find("reading-view").is_none());
                window.input("Kept writing. ", cx);
            })
            .expect("return to editing");
            cx.run_until_parked();
        }
        state.read_with(cx, |state, _| {
            assert_eq!(
                state.notes[&id].note.content,
                "Kept writing. Kept writing. "
            );
            assert_eq!(state.notes[&id].note.title, "");
        });
        assert!(
            store
                .request(Request::Shutdown)
                .recv_blocking()
                .expect("shutdown")
                .is_ok()
        );
        std::fs::remove_file(path).expect("remove fixture");
    }
}
