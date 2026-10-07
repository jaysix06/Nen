use crate::{
    app::AppState,
    models::*,
    theme::{LIST_WIDTH, palette},
    ui::editor::{NoteEditor, note_menu},
};
use gpui_kit::component::{
    button::*,
    input::{Input, InputEvent, InputState},
    menu::ContextMenuExt,
    *,
};
use gpui_kit::{assets::IconName, prelude::FluentBuilder, *};
use std::collections::HashMap;

#[derive(Clone)]
struct DragTab(String);
impl Render for DragTab {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .p_3()
            .bg(palette(cx).selected)
            .rounded_md()
            .child("Move tab")
    }
}

pub struct AppWindow {
    pub state: Entity<AppState>,
    pub search: Entity<InputState>,
    pub editors: HashMap<String, Entity<NoteEditor>>,
    settings_view: Option<Entity<super::settings::SettingsView>>,
    search_selection: usize,
    list_scroll: UniformListScrollHandle,
    search_task: Option<Task<()>>,
    last_active: Option<String>,
    focus: FocusHandle,
    inactive_focus: Option<FocusHandle>,
    _subscriptions: Vec<Subscription>,
}

impl AppWindow {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search notes"));
        let mut view = Self {
            state: state.clone(),
            search: search.clone(),
            editors: HashMap::new(),
            settings_view: None,
            search_selection: 0,
            list_scroll: UniformListScrollHandle::new(),
            search_task: None,
            last_active: None,
            focus: cx.focus_handle(),
            inactive_focus: None,
            _subscriptions: Vec::new(),
        };
        view._subscriptions
            .push(cx.observe_in(&state, window, |view, _, window, cx| {
                let query = view.state.read(cx).query.clone();
                if view.search.read(cx).value().as_ref() != query {
                    view.search
                        .update(cx, |input, cx| input.set_value(query, window, cx));
                }
                cx.notify();
            }));
        let weak = cx.weak_entity();
        let handle = window.window_handle();
        view._subscriptions
            .push(cx.intercept_keystrokes(move |event, window, cx| {
                if window.window_handle() == handle && !window.has_active_dialog(cx) {
                    let _ = weak.update(cx, |view, cx| view.key_down(&event.keystroke, window, cx));
                }
            }));
        view._subscriptions.push(
            cx.subscribe_in(&search, window, |view, input, event, _, cx| {
                if matches!(event, InputEvent::Change) {
                    let query = input.read(cx).value().to_string();
                    view.search_selection = 0;
                    view.list_scroll.scroll_to_item(0, ScrollStrategy::Top);
                    view.state.update(cx, |state, _| state.query = query);
                    view.search_task = Some(cx.spawn(async move |view, cx| {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(80))
                            .await;
                        let _ = view.update(cx, |view, cx| {
                            view.state.update(cx, |state, cx| state.refresh(cx))
                        });
                    }));
                }
            }),
        );
        view._subscriptions
            .push(cx.observe_window_bounds(window, |view, window, cx| {
                let bounds = window.window_bounds().get_bounds();
                view.state.update(cx, |state, cx| {
                    state.session.maximized =
                        matches!(window.window_bounds(), WindowBounds::Maximized(_));
                    state.session.width = bounds.size.width.into();
                    state.session.height = bounds.size.height.into();
                    state.session.x = Some(bounds.origin.x.into());
                    state.session.y = Some(bounds.origin.y.into());
                    state.save_session(cx);
                });
            }));
        view._subscriptions
            .push(cx.observe_window_activation(window, |view, window, cx| {
                if window.is_window_active() {
                    if let Some(focus) = view.inactive_focus.take() {
                        window.focus(&focus, cx);
                    }
                } else {
                    view.inactive_focus = window.focused(cx);
                    window.blur(cx);
                    // Focus listeners run during drawing. Hidden windows need
                    // one final draw to stop their editor's caret timer.
                    window.defer(cx, |window, cx| window.draw(cx).clear(cx));
                }
                cx.notify();
            }));
        let close_state = state.clone();
        window.on_window_should_close(cx, move |window, cx| {
            close_state.update(cx, |state, cx| {
                if state.settings.minimize_to_tray
                    && state.desktop.as_ref().is_some_and(|d| d.tray.is_some())
                {
                    crate::platform::hide(window, cx);
                } else {
                    state.quit(cx);
                }
            });
            false
        });
        window.focus(&view.focus, cx);
        view
    }

    pub fn command(&mut self, command: &str, window: &mut Window, cx: &mut Context<Self>) {
        match command {
            "new_note" => {
                self.state.update(cx, |state, cx| {
                    state.create_note(false, cx);
                });
            }
            "settings" => {
                let closing = self.state.read(cx).settings_page.is_some();
                self.state.update(cx, |state, cx| {
                    if closing {
                        state.settings_page = None;
                        state.shortcut_capture = None;
                        cx.notify();
                    } else {
                        state.desktop_command("settings", cx);
                    }
                });
                if closing {
                    if let Some(editor) = self
                        .state
                        .read(cx)
                        .session
                        .active
                        .as_ref()
                        .and_then(|id| self.editors.get(id))
                    {
                        editor.update(cx, |editor, cx| editor.focus_body(window, cx));
                    } else {
                        window.focus(&self.focus, cx);
                    }
                }
            }
            "toggle_float" | "quick_note" | "open_app" => self
                .state
                .update(cx, |state, cx| state.desktop_command(command, cx)),
            "close_tab" => {
                let id = self.state.read(cx).session.active.clone();
                if let Some(id) = id {
                    self.state.update(cx, |state, cx| state.close_tab(&id, cx));
                }
            }
            "reopen_tab" => self.state.update(cx, |state, cx| state.reopen_tab(cx)),
            "next_tab" => self.state.update(cx, |state, cx| state.switch_tab(1, cx)),
            "previous_tab" => self.state.update(cx, |state, cx| state.switch_tab(-1, cx)),
            "search" => {
                self.state.update(cx, |state, cx| {
                    state.navigate(Collection::All, cx);
                    state.session.sidebar_hidden = false;
                    state.save_session(cx);
                });
                self.search.update(cx, |input, cx| input.focus(window, cx));
            }
            "toggle_sidebar" => self.state.update(cx, |state, cx| {
                state.session.sidebar_hidden = !state.session.sidebar_hidden;
                state.save_session(cx);
                cx.notify();
            }),
            "find" => {
                if let Some(id) = self.state.read(cx).session.active.as_ref()
                    && let Some(editor) = self.editors.get(id)
                {
                    editor.update(cx, |editor, cx| editor.find(window, cx));
                }
            }
            "toggle_reading" => {
                if self.state.read(cx).settings_page.is_none()
                    && self.state.read(cx).collection != Collection::Reminders
                    && let Some(id) = self.state.read(cx).session.active.as_ref()
                    && let Some(editor) = self.editors.get(id)
                {
                    editor.update(cx, |editor, cx| editor.toggle_reading(window, cx));
                }
            }
            "reminder" => {
                if let Some(id) = self.state.read(cx).session.active.clone() {
                    super::reminders::open_picker(self.state.clone(), id, window, cx);
                }
            }
            "pin" | "archive" => {
                let id = self.state.read(cx).session.active.clone();
                if let Some(id) = id {
                    self.state.update(cx, |state, cx| {
                        if command == "pin" {
                            state.toggle_pin(&id, cx)
                        } else {
                            state.archive(&id, cx)
                        }
                    });
                }
            }
            command if command.starts_with("category_") => {
                if let Some(number) = command
                    .strip_prefix("category_")
                    .and_then(|n| n.parse::<usize>().ok())
                {
                    self.state
                        .update(cx, |state, cx| state.select_category_number(number, cx));
                }
            }
            command if command.starts_with("tab_") => {
                if let Some(index) = command
                    .strip_prefix("tab_")
                    .and_then(|n| n.parse::<usize>().ok())
                    && let Some(id) = self
                        .state
                        .read(cx)
                        .session
                        .tabs
                        .get(index.saturating_sub(1))
                        .cloned()
                {
                    self.state.update(cx, |state, cx| state.open_note(&id, cx));
                }
            }
            _ => {}
        }
    }

    fn key_down(&mut self, keystroke: &Keystroke, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.read(cx).shortcut_capture.is_some() {
            return;
        }
        if self.search.read(cx).focus_handle(cx).is_focused(window)
            && !keystroke.modifiers.modified()
        {
            let count = self.state.read(cx).summaries.len();
            match keystroke.key.as_str() {
                "down" => {
                    self.search_selection =
                        (self.search_selection + 1).min(count.saturating_sub(1));
                    self.list_scroll
                        .scroll_to_item(self.search_selection, ScrollStrategy::Center);
                    cx.notify();
                    cx.stop_propagation();
                    return;
                }
                "up" => {
                    self.search_selection = self.search_selection.saturating_sub(1);
                    self.list_scroll
                        .scroll_to_item(self.search_selection, ScrollStrategy::Center);
                    cx.notify();
                    cx.stop_propagation();
                    return;
                }
                "enter" => {
                    if let Some(id) = self
                        .state
                        .read(cx)
                        .summaries
                        .get(self.search_selection)
                        .map(|n| n.id.clone())
                    {
                        self.state.update(cx, |state, cx| state.open_note(&id, cx));
                        if let Some(editor) = self.editors.get(&id) {
                            editor.update(cx, |editor, cx| editor.focus_body(window, cx));
                        } else {
                            window.focus(&self.focus, cx);
                        }
                    }
                    cx.stop_propagation();
                    return;
                }
                _ => {}
            }
        }
        let shortcut = crate::platform::shortcut_string(keystroke, cx);
        let command = self
            .state
            .read(cx)
            .settings
            .shortcuts
            .iter()
            .find(|(_, binding)| **binding == shortcut)
            .map(|(action, _)| action.clone());
        if let Some(command) = command {
            self.command(&command, window, cx);
            cx.stop_propagation();
        }
    }

    fn note_list(&self, width: f32, window: &Window, cx: &App) -> AnyElement {
        let p = crate::theme::surfaces(&self.state.read(cx).settings, cx);
        let app = self.state.read(cx);
        let mut list = div()
            .id("note-list")
            .test_support()
            .v_flex()
            .w_full()
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .py_2();
        if !app.summaries.is_empty() {
            let list_state = self.state.clone();
            let search_focused = self.search.read(cx).focus_handle(cx).is_focused(window);
            let selection = self.search_selection;
            list = list.child(
                uniform_list(
                    "notes-virtual-list",
                    app.summaries.len(),
                    move |range, _, cx| {
                        let app = list_state.read(cx);
                        let mut rows = Vec::new();
                        for (index, note) in app
                            .summaries
                            .iter()
                            .enumerate()
                            .skip(range.start)
                            .take(range.len())
                        {
                            let state = list_state.clone();
                            let id = note.id.clone();
                            let menu_state = state.clone();
                            let menu_id = id.clone();
                            let selected = if search_focused {
                                index == selection
                            } else {
                                app.session.active.as_ref() == Some(&note.id)
                            };
                            let preview = if note.preview.trim().is_empty() {
                                "Empty note".into()
                            } else {
                                note.preview.lines().take(2).collect::<Vec<_>>().join(" ")
                            };
                            let mut meta = div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .text_xs()
                                .text_color(p.muted)
                                .child(date_label(note.updated_at));
                            if note.is_pinned {
                                meta = meta.child(Icon::new(IconName::Pin).size_3());
                            }
                            if note.reminder_at.is_some() {
                                meta = meta
                                    .child(Icon::new(IconName::Bell).size_3().text_color(p.accent));
                            }
                            rows.push(
                                div()
                                    .id(SharedString::from(format!("note-{}", note.id)))
                                    .test_support()
                                    .v_flex()
                                    .w_full()
                                    .h(px(82.))
                                    .gap_1()
                                    .px_4()
                                    .py_2()
                                    .cursor_pointer()
                                    .when(selected, |row| row.bg(p.selected))
                                    .hover(|row| row.bg(p.selected))
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .truncate()
                                            .child(note.title.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(p.muted)
                                            .truncate()
                                            .max_h(px(18.))
                                            .overflow_hidden()
                                            .child(preview),
                                    )
                                    .child(meta)
                                    .on_click(move |_, _, cx| {
                                        state.update(cx, |state, cx| state.open_note(&id, cx))
                                    })
                                    .context_menu(move |menu, _, cx| {
                                        note_menu(menu, menu_state.clone(), menu_id.clone(), cx)
                                    })
                                    .into_any_element(),
                            );
                        }
                        rows
                    },
                )
                .track_scroll(&self.list_scroll)
                .w_full()
                .flex_1()
                .min_h_0(),
            );
        }
        if app.summaries.is_empty() {
            list = list.child(div().p_5().text_sm().text_color(p.muted).child(
                if app.query.is_empty() {
                    "No notes here yet"
                } else {
                    "No matching notes"
                },
            ));
        }
        div()
            .w(px(width))
            .h_full()
            .flex_shrink_0()
            .overflow_hidden()
            .v_flex()
            .border_r_1()
            .border_color(p.line)
            .bg(p.sidebar)
            .child(
                div()
                    .id("notes-header")
                    .test_support()
                    .h(px(38.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(p.line)
                    .child(super::categories::picker(
                        self.state.clone(),
                        width - 39.,
                        cx,
                    ))
                    .child(
                        Button::new("new-sidebar-note")
                            .ghost()
                            .small()
                            .h_full()
                            .w(px(38.))
                            .flex_shrink_0()
                            .rounded(ButtonRounded::None)
                            .icon(IconName::Plus)
                            .tooltip("New note")
                            .on_click({
                                let state = self.state.clone();
                                move |_, _, cx| {
                                    state.update(cx, |state, cx| {
                                        state.create_note(false, cx);
                                    });
                                }
                            }),
                    ),
            )
            .child(list)
            .when_some(app.update.as_ref(), |view, update| {
                let status = app.update_status;
                view.child(
                    Button::new("sidebar-update")
                        .ghost()
                        .w_full()
                        .h_auto()
                        .px_4()
                        .py_3()
                        .rounded(ButtonRounded::None)
                        .disabled(status.is_some())
                        .accessibility_label("Update Nen and restart")
                        .tooltip("Save your notes, install the latest version, and reopen Nen")
                        .child(
                            div()
                                .id("update-banner")
                                .test_support()
                                .w_full()
                                .v_flex()
                                .gap_1()
                                .text_left()
                                .whitespace_normal()
                                .child(
                                    div().text_sm().text_color(p.accent).child(
                                        status.unwrap_or("A new version of Nen is available"),
                                    ),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(p.muted)
                                        .child(format!("Update to {} and restart", update.version)),
                                ),
                        )
                        .on_click({
                            let state = self.state.clone();
                            move |_, _, cx| state.update(cx, |state, cx| state.install_update(cx))
                        }),
                )
            })
            .child(
                div()
                    .id("notes-footer")
                    .test_support()
                    .h(px(38.))
                    .flex_shrink_0()
                    .border_t_1()
                    .border_color(p.line)
                    .flex()
                    .items_center()
                    .child(
                        Button::new("sidebar-settings")
                            .ghost()
                            .small()
                            .w_full()
                            .h_full()
                            .px_4()
                            .rounded(ButtonRounded::None)
                            .accessibility_label("Settings")
                            .child(
                                div()
                                    .w_full()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(Icon::new(IconName::Settings).size_3())
                                    .child("Settings"),
                            )
                            .on_click({
                                let state = self.state.clone();
                                move |_, _, cx| {
                                    state.update(cx, |state, cx| {
                                        state.desktop_command("settings", cx)
                                    })
                                }
                            }),
                    ),
            )
            .into_any_element()
    }

    fn tabs(&self, window: &mut Window, cx: &mut App) -> AnyElement {
        let p = crate::theme::surfaces(&self.state.read(cx).settings, cx);
        let selected = self
            .state
            .read(cx)
            .session
            .active
            .as_ref()
            .and_then(|id| {
                self.state
                    .read(cx)
                    .session
                    .tabs
                    .iter()
                    .position(|tab| tab == id)
            })
            .unwrap_or(0);
        let underline = gpui_kit::base::motion::spring(
            "active-tab-line",
            selected as f32 * 164.,
            gpui_kit::base::motion::Spring::new(std::time::Duration::from_millis(160)),
            window,
            cx,
        );
        let app = self.state.read(cx);
        let mut tabs = div()
            .id("tabs")
            .test_support()
            .h(px(38.))
            .flex_shrink_0()
            .w_full()
            .flex()
            .items_center()
            .overflow_x_scroll()
            .bg(p.canvas)
            .border_b_1()
            .border_color(p.line);
        for id in &app.session.tabs {
            let title = app
                .notes
                .get(id)
                .map(|buffer| buffer.note.display_title())
                .unwrap_or("Untitled");
            let active = app.session.active.as_ref() == Some(id);
            let state = self.state.clone();
            let note_id = id.clone();
            let close_state = state.clone();
            let close_id = id.clone();
            let middle_state = state.clone();
            let middle_id = id.clone();
            let drop_state = state.clone();
            let drop_id = id.clone();
            let menu_state = state.clone();
            let menu_id = id.clone();
            tabs = tabs.child(
                div()
                    .id(SharedString::from(format!("tab-{id}")))
                    .h_full()
                    .w(px(164.))
                    .flex_shrink_0()
                    .px_3()
                    .gap_2()
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .border_r_1()
                    .border_color(p.line)
                    .when(active, |tab| tab.bg(p.paper))
                    .hover(|tab| tab.bg(p.paper))
                    .on_drag(DragTab(id.clone()), |value, _, _, cx| {
                        cx.new(|_| value.clone())
                    })
                    .on_drop(move |value: &DragTab, _, cx| {
                        drop_state.update(cx, |state, cx| state.reorder_tab(&value.0, &drop_id, cx))
                    })
                    .on_mouse_down(MouseButton::Middle, move |_, _, cx| {
                        middle_state.update(cx, |state, cx| state.close_tab(&middle_id, cx));
                        cx.stop_propagation();
                    })
                    .child(
                        div()
                            .flex_1()
                            .truncate()
                            .text_size(px(13.))
                            .child(title.to_owned()),
                    )
                    .child(
                        Button::new(SharedString::from(format!("close-{id}")))
                            .ghost()
                            .xsmall()
                            .icon(IconName::X)
                            .tooltip("Close tab")
                            .on_click(move |_, _, cx| {
                                cx.stop_propagation();
                                close_state.update(cx, |state, cx| state.close_tab(&close_id, cx));
                            }),
                    )
                    .on_click(move |_, _, cx| {
                        state.update(cx, |state, cx| state.open_note(&note_id, cx))
                    })
                    .context_menu(move |menu, _, cx| {
                        note_menu(menu, menu_state.clone(), menu_id.clone(), cx)
                    }),
            );
        }
        let state = self.state.clone();
        tabs.when(!app.session.tabs.is_empty(), |tabs| {
            tabs.child(
                div()
                    .absolute()
                    .left(px(underline))
                    .bottom_0()
                    .w(px(164.))
                    .h(px(2.))
                    .bg(p.accent),
            )
            .child(
                Button::new("add-tab")
                    .ghost()
                    .small()
                    .icon(IconName::Plus)
                    .tooltip("New note · Ctrl+N")
                    .on_click(move |_, _, cx| {
                        state.update(cx, |state, cx| {
                            state.create_note(false, cx);
                        });
                    }),
            )
        })
        .into_any_element()
    }
}

impl Render for AppWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let config = self.state.read(cx).settings.clone();
        let p = crate::theme::surfaces(&config, cx);
        let active = self.state.read(cx).session.active.clone();
        let loaded = active
            .as_ref()
            .is_some_and(|id| self.state.read(cx).notes.contains_key(id));
        if let Some(id) = active.clone()
            && loaded
            && !self.editors.contains_key(&id)
        {
            let state = self.state.clone();
            let editor_id = id.clone();
            let editor = cx.new(|cx| NoteEditor::new(state, editor_id, window, cx));
            self.editors.insert(id, editor);
        }
        let editor = active.as_ref().and_then(|id| self.editors.get(id)).cloned();
        if window.is_window_active()
            && active != self.last_active
            && let Some(editor) = editor.as_ref()
        {
            if !self.search.read(cx).focus_handle(cx).is_focused(window) {
                editor.update(cx, |editor, cx| editor.focus_body(window, cx));
            }
            self.last_active = active.clone();
        }
        if window.is_window_active()
            && let Some(id) = self.state.read(cx).focus_title.clone()
            && let Some(editor) = self.editors.get(&id)
        {
            editor.update(cx, |editor, cx| editor.focus_title(window, cx));
            self.state.update(cx, |state, _| state.focus_title = None);
        }
        let open_tabs = self.state.read(cx).session.tabs.clone();
        self.editors.retain(|id, _| open_tabs.contains(id));
        let content = if self.state.read(cx).collection == Collection::Reminders {
            super::reminders::reminders_page(&self.state, cx)
        } else if let Some(editor) = editor {
            editor.into_any_element()
        } else {
            let state = self.state.clone();
            div()
                .flex_1()
                .v_flex()
                .items_center()
                .justify_center()
                .gap_2()
                .bg(p.paper)
                .child(div().text_sm().text_color(p.muted).child("No note open"))
                .child(
                    Button::new("first-note")
                        .primary()
                        .label("New note")
                        .on_click(move |_, _, cx| {
                            state.update(cx, |state, cx| {
                                state.create_note(false, cx);
                            });
                        }),
                )
                .into_any_element()
        };
        let settings_open = self.state.read(cx).settings_page.is_some();
        if !settings_open {
            self.settings_view = None;
        }
        if settings_open && self.settings_view.is_none() {
            let state = self.state.clone();
            self.settings_view =
                Some(cx.new(|cx| super::settings::SettingsView::new(state, window, cx)));
        }
        let sidebar_width = gpui_kit::base::motion::spring(
            "notes-sidebar-width",
            if self.state.read(cx).session.sidebar_hidden {
                0.
            } else {
                LIST_WIDTH
            },
            gpui_kit::base::motion::Spring::new(std::time::Duration::from_millis(200)),
            window,
            cx,
        );
        let body = if settings_open {
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .children(self.settings_view.clone())
                .into_any_element()
        } else {
            let editor = div()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .overflow_hidden()
                .v_flex()
                .when(
                    self.state.read(cx).collection != Collection::Reminders,
                    |view| view.child(self.tabs(window, cx)),
                )
                .child(content);
            div()
                .flex()
                .flex_1()
                .min_h_0()
                .when(sidebar_width > 0.5, |view| {
                    view.child(self.note_list(sidebar_width, window, cx))
                })
                .child(editor)
                .into_any_element()
        };
        let background =
            super::wallpaper::wallpaper(&config, self.state.read(cx).background.clone(), 0.);
        let mut root = div()
            .id("app")
            .key_context("Nen")
            .track_focus(&self.focus)
            .size_full()
            .v_flex()
            .bg(p.canvas)
            .text_color(p.text)
            .font_family("Segoe UI")
            .text_size(px(14.))
            .child(background)
            .child(
                TitleBar::new()
                    .h(px(40.))
                    .bg(p.canvas)
                    .pl_3()
                    .border_b_1()
                    .border_color(p.line)
                    .child(
                        div()
                            .h_full()
                            .flex_1()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div().occlude().child(
                                    Button::new("toggle-sidebar")
                                        .ghost()
                                        .small()
                                        .icon(IconName::PanelLeft)
                                        .tooltip("Toggle notes sidebar")
                                        .on_click(cx.listener(|view, _, window, cx| {
                                            view.command("toggle_sidebar", window, cx)
                                        })),
                                ),
                            )
                            .child(div().flex_1())
                            .child(
                                div().occlude().mr_3().child(
                                    Input::new(&self.search)
                                        .prefix(Icon::new(IconName::Search).size_3())
                                        .w(px(228.))
                                        .small()
                                        .aria_label("Search notes"),
                                ),
                            ),
                    ),
            )
            .child(body);
        if let Some(message) = self.state.read(cx).message.clone() {
            root = root.child(
                div()
                    .absolute()
                    .bottom_5()
                    .right_5()
                    .px_4()
                    .py_3()
                    .rounded_md()
                    .bg(p.text)
                    .text_color(p.paper)
                    .text_sm()
                    .child(message),
            );
        }
        if let Some(error) = self.state.read(cx).error.clone() {
            let retry = self.state.clone();
            let dismiss = self.state.clone();
            root = root.child(
                div()
                    .absolute()
                    .bottom_5()
                    .left(px(200.))
                    .right_5()
                    .p_4()
                    .rounded_md()
                    .bg(rgb(0x7d302b))
                    .text_color(rgb(0xffffff))
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(div().flex_1().text_sm().child(error))
                    .child(
                        Button::new("retry-storage")
                            .ghost()
                            .small()
                            .label("Retry saving")
                            .on_click(move |_, _, cx| {
                                retry.update(cx, |state, cx| state.retry_saving(cx))
                            }),
                    )
                    .child(
                        Button::new("dismiss-error")
                            .ghost()
                            .small()
                            .icon(IconName::X)
                            .tooltip("Dismiss")
                            .on_click(move |_, _, cx| {
                                dismiss.update(cx, |state, cx| {
                                    state.error = None;
                                    cx.notify();
                                })
                            }),
                    ),
            );
        }
        root
    }
}
