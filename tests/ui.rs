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
fn short_and_long_notes_fill_the_sidebar_and_accept_edge_clicks(cx: &mut TestAppContext) {
    cx.dispatcher.allow_parking();
    let path =
        std::env::temp_dir().join(format!("still-row-width-{}.sqlite", uuid::Uuid::new_v4()));
    let ids = {
        let db = Database::open(&path).expect("database");
        [
            "A",
            "A much longer note title that must truncate inside the sidebar",
        ]
        .map(|title| {
            let mut note = still::models::Note::new(still::models::NoteType::Normal);
            note.title = title.into();
            db.save_note(&note).expect("note");
            note.id
        })
    };
    let (store, _, _, _) = Store::start(path.clone()).expect("worker");
    cx.update(gpui_kit::init);
    let state = cx.new(|cx| {
        AppState::new(
            store.clone(),
            Settings {
                default_wallpaper: false,
                reduced_motion: true,
                ..Settings::default()
            },
            Session::default(),
            cx,
        )
    });
    let (handle, _) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| AppWindow::new(state.clone(), window, cx))
        })
        .expect("window")
    });
    store
        .request(Request::Categories)
        .recv_blocking()
        .expect("load barrier")
        .expect("categories");
    cx.run_until_parked();
    for id in ids {
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let list = window.find("note-list").bounds();
            let row_id = gpui_kit::SharedString::from(format!("note-{id}"));
            let row = window.find(row_id.clone()).bounds();
            assert!((row.left() - list.left()).abs() < gpui_kit::px(1.));
            assert!(
                (row.right() - list.right()).abs() < gpui_kit::px(1.),
                "Every row must fill the sidebar, even with a short title: {row:?} / {list:?}"
            );
            window.click_at(
                row_id,
                gpui_kit::point(row.size.width - gpui_kit::px(3.), gpui_kit::px(20.)),
                cx,
            );
        })
        .expect("full-width row");
        cx.run_until_parked();
        assert_eq!(
            state.read_with(cx, |state, _| state.session.active.clone()),
            Some(id)
        );
    }
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let header = window.find("notes-header").bounds();
        let picker = window.find("category-picker").bounds();
        let create = window.find("new-sidebar-note").bounds();
        assert!((picker.left() - header.left()).abs() < gpui_kit::px(1.));
        assert!((picker.right() - create.left()).abs() < gpui_kit::px(1.));
        window.click_at(
            "category-picker",
            gpui_kit::point(picker.size.width - gpui_kit::px(3.), gpui_kit::px(16.)),
            cx,
        );
        window.render_frame(cx);
        let menu = window.find("popup-menu").bounds();
        for index in [0usize, 1, 3, 4, 5, 7, 8, 10] {
            let row = window.within("popup-menu").find(index).bounds();
            assert!((menu.right() - row.right()).abs() <= gpui_kit::px(5.));
            assert!((row.left() - menu.left()).abs() <= gpui_kit::px(5.));
        }
        window.press("escape", cx);
        window.render_frame(cx);
        let footer = window.find("notes-footer").bounds();
        let settings = window.find("sidebar-settings").bounds();
        assert!((settings.left() - footer.left()).abs() < gpui_kit::px(1.));
        assert!((settings.right() - footer.right()).abs() < gpui_kit::px(1.));
        window.click_at(
            "sidebar-settings",
            gpui_kit::point(settings.size.width - gpui_kit::px(3.), gpui_kit::px(16.)),
            cx,
        );
        window.render_frame(cx);
        let sidebar = window.find("settings-sidebar").bounds();
        for name in [
            "General",
            "Reminders",
            "Appearance",
            "Background",
            "Floating bar",
            "Shortcuts",
            "Advanced",
            "close-settings",
        ] {
            let row = window.find(name).bounds();
            assert!((row.left() - sidebar.left()).abs() < gpui_kit::px(1.));
            assert!((row.right() - sidebar.right()).abs() <= gpui_kit::px(1.));
        }
        let row = window.find("Appearance").bounds();
        window.click_at(
            "Appearance",
            gpui_kit::point(row.size.width - gpui_kit::px(3.), gpui_kit::px(16.)),
            cx,
        );
    })
    .expect("full-width navigation controls");
    assert_eq!(
        state.read_with(cx, |state, _| state.settings_page.clone()),
        Some("Appearance".into())
    );
    store
        .request(Request::Shutdown)
        .recv_blocking()
        .expect("shutdown")
        .expect("flush");
    std::fs::remove_file(path).expect("cleanup");
}

#[gpui_kit::test]
fn closing_empty_drafts_discards_them_without_reopening_or_late_saves(cx: &mut TestAppContext) {
    cx.dispatcher.allow_parking();
    let path = std::env::temp_dir().join(format!("still-empty-{}.sqlite", uuid::Uuid::new_v4()));
    let (store, _, _, _) = Store::start(path.clone()).expect("worker");
    cx.update(gpui_kit::init);
    let state = cx.new(|cx| {
        AppState::new(
            store.clone(),
            Settings {
                default_wallpaper: false,
                reduced_motion: true,
                ..Settings::default()
            },
            Session::default(),
            cx,
        )
    });
    let (handle, _) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| AppWindow::new(state.clone(), window, cx))
        })
        .expect("window")
    });
    for whitespace in [false, true] {
        cx.simulate_keystrokes(handle, "ctrl-n");
        cx.run_until_parked();
        let id = state
            .read_with(cx, |state, _| state.session.active.clone())
            .expect("draft");
        assert!(matches!(
            store
                .request(Request::Load(id.clone()))
                .recv_blocking()
                .expect("draft barrier")
                .expect("draft query"),
            still::storage::Response::Note(None)
        ));
        if whitespace {
            state.update(cx, |state, cx| {
                state.edit(&id, "  ".into(), "\n\t".into(), cx);
                state.save_now(&id, cx);
            });
        }
        cx.simulate_keystrokes(handle, "ctrl-w");
        store
            .request(Request::Load(id.clone()))
            .recv_blocking()
            .expect("delete barrier")
            .expect("deleted");
        cx.run_until_parked();
        state.update(cx, |state, cx| state.save_now(&id, cx));
        cx.simulate_keystrokes(handle, "ctrl-shift-t");
        cx.run_until_parked();
        assert!(state.read_with(cx, |state, _| state.session.tabs.is_empty()
            && !state.notes.contains_key(&id)
            && state.closed_tabs.is_empty()
            && state.message.is_none()));
    }
    for (title, content) in [("Title only", ""), ("", "Body only")] {
        let id = state.update(cx, |state, cx| {
            let id = state.create_note(false, cx);
            state.edit(&id, title.into(), content.into(), cx);
            state.close_tab(&id, cx);
            id
        });
        assert!(matches!(
            store
                .request(Request::Load(id))
                .recv_blocking()
                .expect("save barrier")
                .expect("load"),
            still::storage::Response::Note(Some(_))
        ));
        cx.run_until_parked();
    }
    store
        .request(Request::Shutdown)
        .recv_blocking()
        .expect("shutdown")
        .expect("flush");
    let database = Database::open(&path).expect("reopen");
    assert_eq!(
        database
            .list(still::models::Collection::All, "")
            .expect("list")
            .len(),
        2
    );
    drop(database);
    std::fs::remove_file(path).expect("cleanup");
}

#[gpui_kit::test]
fn editor_font_slider_changes_rendered_text_size(cx: &mut TestAppContext) {
    cx.dispatcher.allow_parking();
    let path = std::env::temp_dir().join(format!("still-font-{}.sqlite", uuid::Uuid::new_v4()));
    let id = {
        let database = Database::open(&path).expect("database");
        let mut note = still::models::Note::new(still::models::NoteType::Normal);
        note.title = "Font test".into();
        note.content = "First line\nSecond line".into();
        database.save_note(&note).expect("note");
        note.id
    };
    let (store, _, _, _) = Store::start(path.clone()).expect("worker");
    let settings = Settings {
        default_wallpaper: false,
        reduced_motion: true,
        editor_font_size: 14.,
        ..Settings::default()
    };
    let session = Session {
        tabs: vec![id.clone()],
        active: Some(id.clone()),
        ..Session::default()
    };
    cx.update(gpui_kit::init);
    cx.update(|cx| still::theme::apply(&settings, cx));
    let state = cx.new(|cx| AppState::new(store.clone(), settings, session, cx));
    let (handle, root) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| AppWindow::new(state.clone(), window, cx))
        })
        .expect("window")
    });
    store
        .request(Request::Categories)
        .recv_blocking()
        .expect("load barrier")
        .expect("categories");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window
                .within("notes-header")
                .find("category-picker")
                .visible()
        );
    })
    .expect("editor frame");
    let body = root.read_with(cx, |root, cx| root.editors[&id].read(cx).body.clone());
    let before = body.read_with(cx, |body, _| {
        body.line_height().expect("initial text layout")
    });
    cx.simulate_keystrokes(handle, "ctrl-,");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("Appearance", cx);
        window.render_frame(cx);
        let row = window.find("settings-nav-Appearance").bounds();
        let label = window.find("settings-nav-label-Appearance").bounds();
        assert!(
            label.left() - row.left() <= gpui_kit::px(28.),
            "Preferences labels must be left aligned"
        );
        window.click_at(
            "settings-slider-font",
            gpui_kit::point(gpui_kit::px(155.), gpui_kit::px(8.)),
            cx,
        );
    })
    .expect("change font slider");
    cx.run_until_parked();
    assert!(state.read_with(cx, |state, _| state.settings.editor_font_size) > 24.);
    cx.simulate_keystrokes(handle, "ctrl-,");
    cx.run_until_parked();
    assert!(state.read_with(cx, |state, _| state.settings_page.is_none()));
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .expect("updated editor frame");
    let after = body.read_with(cx, |body, _| {
        body.line_height().expect("updated text layout")
    });
    assert!(
        after > before * 1.5,
        "Font size must change rendered text: {before:?} -> {after:?}"
    );
    cx.update_window(handle, |_, window, cx| {
        window.input("Continued writing.", cx)
    })
    .expect("focus returns to the editor");
    cx.run_until_parked();
    assert!(state.read_with(cx, |state, _| {
        state.notes[&id].note.content.contains("Continued writing.")
    }));
    cx.simulate_keystrokes(handle, "ctrl-,");
    cx.run_until_parked();
    assert_eq!(
        state.read_with(cx, |state, _| state.settings_page.clone()),
        Some("General".into())
    );
    cx.simulate_keystrokes(handle, "ctrl-,");
    cx.run_until_parked();
    assert!(state.read_with(cx, |state, _| state.settings_page.is_none()));
    store
        .request(Request::Shutdown)
        .recv_blocking()
        .expect("shutdown")
        .expect("flush");
    std::fs::remove_file(path).expect("cleanup");
}

#[gpui_kit::test]
fn dim_slider_changes_tint_and_overlay_covers_the_image(cx: &mut TestAppContext) {
    cx.dispatcher.allow_parking();
    let path = std::env::temp_dir().join(format!("still-dim-{}.sqlite", uuid::Uuid::new_v4()));
    let (store, _, _, _) = Store::start(path.clone()).expect("database");
    let settings = Settings {
        default_wallpaper: false,
        background_dim: 0.,
        reduced_motion: true,
        ..Settings::default()
    };
    cx.update(gpui_kit::init);
    cx.update(|cx| still::theme::apply(&settings, cx));
    let state = cx.new(|cx| AppState::new(store.clone(), settings, Session::default(), cx));
    state.update(cx, |state, _| {
        state.background =
            Some(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/still-lake.png"));
        state.settings_page = Some("Background".into());
    });
    let (handle, _) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| AppWindow::new(state.clone(), window, cx))
        })
        .expect("window")
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("wallpaper-dim").bounds(),
            window.find("wallpaper-layer").bounds(),
            "Tint must cover the image rather than follow it in layout"
        );
        window.click_at(
            "settings-slider-dim",
            gpui_kit::point(gpui_kit::px(42.), gpui_kit::px(8.)),
            cx,
        );
    })
    .expect("adjust Dim");
    cx.run_until_parked();
    state.read_with(cx, |state, _| {
        let dim = state.settings.background_dim;
        assert!(
            dim > 0. && dim < 0.58,
            "Low Dim changes must reach settings: {dim}"
        );
        assert_eq!(still::theme::background_dim(&state.settings), dim);
    });
    store
        .request(Request::Shutdown)
        .recv_blocking()
        .expect("shutdown")
        .expect("flush");
    std::fs::remove_file(path).expect("cleanup");
}

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
            for dim in [0., 0.05, 0.2, 0.35, 0.5, 0.65, 0.8] {
                let settings = Settings {
                    theme: theme.into(),
                    surface: surface.into(),
                    background_image: Some("white-test-wallpaper.png".into()),
                    background_dim: dim,
                    ..Settings::default()
                };
                cx.update(|cx| {
                    still::theme::apply(&settings, cx);
                    assert_eq!(
                        still::theme::background_dim(&settings),
                        dim,
                        "{theme}/{surface}: Dim must match the displayed percentage"
                    );
                    let palette = still::theme::surfaces(&settings, cx);
                    // White is the worst backdrop for the dark theme; black for light.
                    let source = if theme == "Dark" {
                        1. - still::theme::background_dim(&settings)
                    } else {
                        0.
                    };
                    for panel in [palette.paper, palette.sidebar, palette.canvas] {
                        let paper = gpui_kit::Rgba::from(panel);
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
                            assert!(
                                contrast >= 4.5,
                                "{theme}/{surface}/{dim}: contrast {contrast}"
                            );
                        }
                    }
                });
            }
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
    // Categories now live inside the notes pane; reveal it to use the picker.
    cx.simulate_keystrokes(window, "ctrl-b");
    cx.run_until_parked();
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
    cx.simulate_keystrokes(window, "ctrl-b");
    cx.run_until_parked();
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
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("note-title", cx);
        window.input("Keep this tab", cx);
    })
    .expect("second note has content to reopen");
    cx.run_until_parked();
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
