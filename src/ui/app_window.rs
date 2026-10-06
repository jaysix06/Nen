use crate::{
    app::AppState,
    models::*,
    theme::{LIST_WIDTH, SIDEBAR_WIDTH, palette},
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
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl AppWindow {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search notes"));
        let mut view = Self {
            state: state.clone(),
            search: search.clone(),
            editors: HashMap::new(),
            focus: cx.focus_handle(),
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
                    view.state.update(cx, |state, cx| state.search(query, cx));
                }
            }),
        );
        view._subscriptions
            .push(cx.observe_window_bounds(window, |view, window, cx| {
                let bounds = window.window_bounds().get_bounds();
                view.state.update(cx, |state, _| {
                    state.session.width = bounds.size.width.into();
                    state.session.height = bounds.size.height.into();
                    state.session.x = Some(bounds.origin.x.into());
                    state.session.y = Some(bounds.origin.y.into());
                    state.save_session();
                });
            }));
        let close_state = state.clone();
        window.on_window_should_close(cx, move |_, cx| {
            close_state.update(cx, |state, cx| state.quit(cx));
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
            "close_tab" => {
                let id = self.state.read(cx).session.active.clone();
                if let Some(id) = id {
                    self.state.update(cx, |state, cx| state.close_tab(&id, cx));
                }
            }
            "reopen_tab" => self.state.update(cx, |state, cx| state.reopen_tab(cx)),
            "next_tab" => self.state.update(cx, |state, cx| state.switch_tab(1, cx)),
            "previous_tab" => self.state.update(cx, |state, cx| state.switch_tab(-1, cx)),
            "search" => self.search.update(cx, |input, cx| input.focus(window, cx)),
            "find" => {
                if let Some(id) = self.state.read(cx).session.active.as_ref()
                    && let Some(editor) = self.editors.get(id)
                {
                    editor.update(cx, |editor, cx| editor.find(window, cx));
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
            _ => {}
        }
    }

    fn key_down(&mut self, keystroke: &Keystroke, window: &mut Window, cx: &mut Context<Self>) {
        let shortcut = keystroke.to_string().to_ascii_lowercase();
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
        } else if keystroke.modifiers.control && !keystroke.modifiers.alt {
            if let Ok(index) = keystroke.key.parse::<usize>()
                && index > 0
                && index <= 9
            {
                let id = self.state.read(cx).session.tabs.get(index - 1).cloned();
                if let Some(id) = id {
                    self.state.update(cx, |state, cx| state.open_note(&id, cx));
                    cx.stop_propagation();
                }
            }
        }
    }

    fn sidebar(&self, cx: &App) -> AnyElement {
        let p = palette(cx);
        let active = self.state.read(cx).collection;
        let mut sidebar = div()
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .flex_shrink_0()
            .v_flex()
            .bg(p.sidebar)
            .px_3()
            .pt_5()
            .pb_3()
            .gap_1()
            .border_r_1()
            .border_color(p.line);
        let state = self.state.clone();
        sidebar = sidebar.child(
            Button::new("new-note")
                .primary()
                .icon(IconName::Plus)
                .label("New note")
                .w_full()
                .mb_5()
                .on_click(move |_, _, cx| {
                    state.update(cx, |state, cx| {
                        state.create_note(false, cx);
                    });
                }),
        );
        for (collection, icon) in [
            (Collection::All, IconName::Notebook),
            (Collection::Pinned, IconName::Pin),
            (Collection::Reminders, IconName::Bell),
            (Collection::Archive, IconName::Archive),
        ] {
            let state = self.state.clone();
            sidebar = sidebar.child(
                Button::new(collection.label())
                    .ghost()
                    .icon(icon)
                    .label(collection.label())
                    .w_full()
                    .justify_start()
                    .when(active == collection, |button| {
                        button.bg(p.selected).text_color(p.accent)
                    })
                    .on_click(move |_, _, cx| {
                        state.update(cx, |state, cx| state.navigate(collection, cx))
                    }),
            );
        }
        sidebar
            .child(div().flex_1())
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(p.muted)
                    .child("Local only"),
            )
            .into_any_element()
    }

    fn note_list(&self, cx: &App) -> AnyElement {
        let p = palette(cx);
        let app = self.state.read(cx);
        let title = if app.query.is_empty() {
            app.collection.label()
        } else {
            "Search results"
        };
        let mut list = div()
            .id("note-list")
            .v_flex()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px_2()
            .py_2();
        for note in &app.summaries {
            let state = self.state.clone();
            let id = note.id.clone();
            let menu_state = state.clone();
            let menu_id = id.clone();
            let selected = app.session.active.as_ref() == Some(&note.id);
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
                meta = meta.child(Icon::new(IconName::Bell).size_3().text_color(p.accent));
            }
            list = list.child(
                div()
                    .id(SharedString::from(format!("note-{}", note.id)))
                    .v_flex()
                    .gap_2()
                    .px_3()
                    .py_3()
                    .mb_1()
                    .rounded(px(5.))
                    .cursor_pointer()
                    .when(selected, |row| row.bg(p.selected))
                    .hover(|row| row.bg(p.selected))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .truncate()
                            .child(note.title.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(p.muted)
                            .max_h(px(37.))
                            .overflow_hidden()
                            .child(preview),
                    )
                    .child(meta)
                    .on_click(move |_, _, cx| {
                        state.update(cx, |state, cx| state.open_note(&id, cx))
                    })
                    .context_menu(move |menu, _, _| {
                        note_menu(menu, menu_state.clone(), menu_id.clone())
                    }),
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
            .w(px(LIST_WIDTH))
            .h_full()
            .flex_shrink_0()
            .v_flex()
            .border_r_1()
            .border_color(p.line)
            .bg(p.canvas)
            .child(
                div()
                    .h(px(50.))
                    .px_5()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(p.line)
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(p.muted)
                            .child(app.summaries.len().to_string()),
                    ),
            )
            .child(list)
            .into_any_element()
    }

    fn tabs(&self, cx: &App) -> AnyElement {
        let p = palette(cx);
        let app = self.state.read(cx);
        let mut tabs = div()
            .id("tabs")
            .h(px(46.))
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
                    .min_w(px(100.))
                    .max_w(px(196.))
                    .px_3()
                    .gap_2()
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .border_r_1()
                    .border_color(p.line)
                    .when(active, |tab| {
                        tab.bg(p.paper).border_b_2().border_color(p.accent)
                    })
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
                    .context_menu(move |menu, _, _| {
                        note_menu(menu, menu_state.clone(), menu_id.clone())
                    }),
            );
        }
        let state = self.state.clone();
        tabs.child(
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
        .into_any_element()
    }
}

impl Render for AppWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
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
        if let Some(id) = self.state.read(cx).focus_title.clone()
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
                .gap_4()
                .bg(p.paper)
                .child(
                    Icon::new(IconName::NotebookPen)
                        .size_8()
                        .text_color(p.muted),
                )
                .child(div().text_lg().child("A little room to think."))
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
        let mut root = div()
            .id("app")
            .key_context("Still")
            .track_focus(&self.focus)
            .size_full()
            .v_flex()
            .bg(p.canvas)
            .text_color(p.text)
            .font_family("Segoe UI")
            .text_size(px(14.))
            .child(
                div()
                    .h(px(58.))
                    .px_5()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(p.line)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(Icon::new(IconName::Notebook).size_5().text_color(p.accent))
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_size(px(19.))
                                    .child("still"),
                            ),
                    )
                    .child(
                        Input::new(&self.search)
                            .prefix(Icon::new(IconName::Search).size_4())
                            .w(px(300.))
                            .small()
                            .aria_label("Search all notes"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.sidebar(cx))
                    .child(self.note_list(cx))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .v_flex()
                            .child(self.tabs(cx))
                            .child(content),
                    ),
            );
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
                    .child(error),
            );
        }
        root
    }
}
