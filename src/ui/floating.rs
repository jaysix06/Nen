use crate::{
    app::AppState,
    models::*,
    platform,
    storage::{Request, Response},
    ui::{editor::NoteEditor, reminders::ReminderPicker},
};
use gpui_kit::component::{
    button::*,
    input::{Input, InputEvent, InputState},
    *,
};
use gpui_kit::{
    assets::IconName,
    base::motion::{Spring, spring},
    prelude::FluentBuilder,
    *,
};
use std::time::Duration;

pub struct FloatingWindow {
    state: Entity<AppState>,
    input: Entity<InputState>,
    expanded: bool,
    results: Vec<NoteSummary>,
    editor: Option<(String, Entity<NoteEditor>)>,
    picker: Option<(String, Entity<ReminderPicker>)>,
    picker_subscription: Option<Subscription>,
    generation: u64,
    selected: usize,
    anchor: (i32, i32, i32, i32),
    last_size: (i32, i32),
    dragging: bool,
    position_override: Option<(i32, i32)>,
    _subscriptions: Vec<Subscription>,
}
impl FloatingWindow {
    #[cfg(feature = "ui-testing")]
    pub fn expand_for_capture(&mut self, cx: &mut Context<Self>) {
        self.expanded = true;
        self.search(cx);
    }
    pub fn invalidate_size(&mut self, cx: &mut Context<Self>) {
        self.last_size = (0, 0);
        self.position_override = None;
        cx.notify();
    }
    fn start_drag(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self.dragging {
            return;
        }
        self.dragging = true;
        let drag = platform::drag_floating(window, cx);
        cx.spawn(async move |view, cx| {
            let position = drag.await;
            let _ = view.update(cx, |view, cx| {
                view.dragging = false;
                if let Some((x, y)) = position {
                    view.anchor = platform::work_area();
                    view.last_size = (0, 0);
                    if view.state.read(cx).settings.floating_remember_position {
                        view.state.update(cx, |state, cx| {
                            let mut settings = state.settings.clone();
                            settings.floating_x = Some(x);
                            settings.floating_y = Some(y);
                            settings.floating_position = "Custom".into();
                            state.update_settings(settings, cx);
                        });
                    } else {
                        view.position_override = Some((x, y));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx));
        let mut view = Self {
            state: state.clone(),
            input: input.clone(),
            expanded: false,
            results: Vec::new(),
            editor: None,
            picker: None,
            picker_subscription: None,
            generation: 0,
            selected: 0,
            anchor: platform::work_area(),
            last_size: (0, 0),
            dragging: false,
            position_override: None,
            _subscriptions: Vec::new(),
        };
        view._subscriptions
            .push(cx.observe(&state, |_, _, cx| cx.notify()));
        view._subscriptions
            .push(cx.subscribe_in(&input, window, |view, _, event, _, cx| {
                if matches!(event, InputEvent::Change) {
                    view.expanded = true;
                    view.search(cx);
                }
            }));
        view._subscriptions
            .push(cx.observe_window_activation(window, |view, window, cx| {
                if window.is_window_active() {
                    view.last_size = (0, 0);
                    cx.notify();
                    if view.state.read(cx).floating_note.is_none() {
                        view.input.update(cx, |input, cx| input.focus(window, cx));
                    } else if view.state.read(cx).floating_reminder {
                        if let Some((_, picker)) = &view.picker {
                            picker.update(cx, |picker, cx| picker.focus_date(window, cx));
                        }
                    } else if let Some((id, editor)) = &view.editor
                        && view.state.read(cx).floating_note.as_ref() == Some(id)
                    {
                        editor.update(cx, |editor, cx| {
                            editor.body.update(cx, |input, cx| input.focus(window, cx))
                        });
                    }
                } else {
                    window.blur(cx);
                    window.defer(cx, |window, cx| window.draw(cx).clear(cx));
                    if view.state.read(cx).settings.floating_hide_on_blur
                        && !window.has_active_dialog(cx)
                    {
                        platform::hide(window, cx);
                        view.state.update(cx, |state, cx| {
                            state.floating_visible = false;
                            cx.notify();
                        });
                    }
                }
            }));
        let weak = cx.weak_entity();
        let handle = window.window_handle();
        view._subscriptions
            .push(cx.intercept_keystrokes(move |event, window, cx| {
                if window.window_handle() == handle && !window.has_active_dialog(cx) {
                    let _ = weak.update(cx, |view, cx| view.key(&event.keystroke, window, cx));
                }
            }));
        let close = state.clone();
        window.on_window_should_close(cx, move |window, cx| {
            platform::hide(window, cx);
            close.update(cx, |state, cx| {
                state.floating_visible = false;
                cx.notify();
            });
            false
        });
        view.search(cx);
        view
    }
    fn search(&mut self, cx: &mut Context<Self>) {
        self.generation += 1;
        let generation = self.generation;
        let response = self.state.read(cx).store.request(Request::List(
            Collection::All,
            self.input.read(cx).value().to_string(),
        ));
        cx.spawn(async move |view, cx| {
            if let Ok(Ok(Response::Notes(notes))) = response.recv().await {
                let _ = view.update(cx, |view, cx| {
                    if view.generation == generation {
                        view.results = notes.into_iter().take(7).collect();
                        view.selected = 0;
                        cx.notify();
                    }
                });
            }
        })
        .detach();
        cx.notify();
    }
    fn open(&mut self, id: String, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            state.load(&id, cx);
            state.floating_note = Some(id);
            state.floating_reminder = false;
            cx.notify();
        });
    }
    fn quick_note(&mut self, cx: &mut Context<Self>) {
        let text = self.input.read(cx).value().to_string();
        self.state.update(cx, |state, cx| {
            let id = state.create_note(true, cx);
            if !text.is_empty() {
                state.edit(&id, text, String::new(), cx);
            }
        });
    }
    fn key(&mut self, key: &Keystroke, window: &mut Window, cx: &mut Context<Self>) {
        let binding = crate::platform::shortcut_string(key, cx);
        let command = self
            .state
            .read(cx)
            .settings
            .shortcuts
            .iter()
            .find(|(_, value)| **value == binding)
            .map(|(action, _)| action.clone());
        if let Some(command) = command {
            match command.as_str() {
                "new_note" => self.quick_note(cx),
                "reminder" => self.state.update(cx, |state, cx| {
                    if state.floating_note.is_some() {
                        state.floating_reminder = true;
                        cx.notify();
                    }
                }),
                "search" => {
                    self.state.update(cx, |state, cx| {
                        state.floating_note = None;
                        state.floating_reminder = false;
                        cx.notify();
                    });
                    self.expanded = true;
                    self.input.update(cx, |input, cx| input.focus(window, cx));
                }
                "find" => {
                    if let Some((_, editor)) = &self.editor {
                        editor.update(cx, |editor, cx| editor.find(window, cx));
                    }
                }
                "next_tab" | "previous_tab" => {
                    if !self.results.is_empty() {
                        let current = self
                            .state
                            .read(cx)
                            .floating_note
                            .as_ref()
                            .and_then(|id| self.results.iter().position(|n| &n.id == id))
                            .unwrap_or(0);
                        let direction = if command == "next_tab" { 1 } else { -1 };
                        let index = (current as isize + direction)
                            .rem_euclid(self.results.len() as isize)
                            as usize;
                        self.open(self.results[index].id.clone(), cx);
                    }
                }
                "toggle_float" | "open_app" | "quick_note" => self
                    .state
                    .update(cx, |state, cx| state.desktop_command(&command, cx)),
                _ => return,
            }
            cx.stop_propagation();
            return;
        }
        if key.key == "escape" {
            if self.state.read(cx).floating_reminder {
                self.state.update(cx, |state, cx| {
                    state.floating_reminder = false;
                    cx.notify();
                });
            } else if self.state.read(cx).floating_note.is_some() {
                self.state.update(cx, |state, cx| {
                    state.floating_note = None;
                    cx.notify();
                });
                self.input.update(cx, |input, cx| input.focus(window, cx));
            } else if self.expanded {
                self.expanded = false;
                self.input
                    .update(cx, |input, cx| input.set_value("", window, cx));
                cx.notify();
            } else {
                platform::hide(window, cx);
                self.state.update(cx, |state, cx| {
                    state.floating_visible = false;
                    cx.notify();
                });
            }
            cx.stop_propagation();
        } else if self.state.read(cx).floating_note.is_none() {
            match key.key.as_str() {
                "down" => {
                    self.expanded = true;
                    self.selected = (self.selected + 1).min(self.results.len().saturating_sub(1));
                    cx.notify();
                    cx.stop_propagation();
                }
                "up" => {
                    self.selected = self.selected.saturating_sub(1);
                    cx.notify();
                    cx.stop_propagation();
                }
                "enter" => {
                    if let Some(note) = self.results.get(self.selected) {
                        self.open(note.id.clone(), cx);
                    } else {
                        self.quick_note(cx);
                    }
                    cx.stop_propagation();
                }
                _ => {}
            }
        }
    }
}
impl Render for FloatingWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = crate::theme::island_palette(
            self.state.read(cx).wallpaper_color,
            &self.state.read(cx).settings,
            cx,
        );
        let mut settings = self.state.read(cx).settings.clone();
        if let Some((x, y)) = self.position_override {
            settings.floating_position = "Custom".into();
            settings.floating_x = Some(x);
            settings.floating_y = Some(y);
        }
        let id = self.state.read(cx).floating_note.clone();
        let reminder = self.state.read(cx).floating_reminder;
        let (target_width, target_height) = if id.is_some() {
            (
                settings.floating_width,
                if reminder {
                    self.picker
                        .as_ref()
                        .map(|(_, picker)| picker.read(cx).preferred_height())
                        .unwrap_or(304.)
                } else {
                    390.
                },
            )
        } else if self.expanded {
            (
                settings.floating_width,
                56. + self.results.len().max(1) as f32 * 64. + 48.,
            )
        } else {
            (settings.floating_width.min(380.), 56.)
        };
        let motion = Spring::new(Duration::from_millis(200)).with_epsilon(0.15);
        let width = spring("island-width", target_width, motion, window, cx);
        let height = spring("island-height", target_height, motion, window, cx);
        let size = (width.round() as i32, height.round() as i32);
        if !self.dragging && self.last_size != size {
            platform::position_floating(window, width, height, &settings, self.anchor, cx);
            self.last_size = size;
        }
        let mut content = div().v_flex().size_full().min_h_0();
        if let Some(id) = id {
            let back = self.state.clone();
            let full = self.state.clone();
            let open_id = id.clone();
            let remind = self.state.clone();
            content = content.child(
                div()
                    .h(px(48.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_3()
                    .border_b_1()
                    .border_color(p.line)
                    .child(
                        Button::new("island-back")
                            .ghost()
                            .small()
                            .text_color(p.text)
                            .icon(IconName::ArrowLeft)
                            .tooltip("Back")
                            .on_click(move |_, _, cx| {
                                back.update(cx, |state, cx| {
                                    state.floating_note = None;
                                    state.floating_reminder = false;
                                    cx.notify();
                                })
                            }),
                    )
                    .child(
                        div()
                            .id("island-drag")
                            .flex_1()
                            .text_sm()
                            .text_color(p.muted)
                            .cursor_move()
                            .child(if reminder { "Reminder" } else { "Quick note" })
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|view, _, window, cx| view.start_drag(window, cx)),
                            ),
                    )
                    .child(
                        Button::new("island-reminder")
                            .ghost()
                            .small()
                            .text_color(p.text)
                            .icon(IconName::Bell)
                            .tooltip("Set reminder")
                            .on_click(move |_, _, cx| {
                                remind.update(cx, |state, cx| {
                                    state.floating_reminder = !state.floating_reminder;
                                    cx.notify();
                                })
                            }),
                    )
                    .child(
                        Button::new("island-open")
                            .ghost()
                            .small()
                            .text_color(p.text)
                            .icon(IconName::ArrowUpRight)
                            .tooltip("Open in full app")
                            .on_click(move |_, window, cx| {
                                platform::hide(window, cx);
                                full.update(cx, |state, cx| {
                                    state.floating_visible = false;
                                    state.floating_note = None;
                                    state.floating_reminder = false;
                                    state.open_note(&open_id, cx);
                                    state.desktop_command("open_app", cx);
                                })
                            }),
                    )
                    .child(
                        Button::new("island-hide")
                            .ghost()
                            .small()
                            .text_color(p.text)
                            .icon(IconName::X)
                            .tooltip("Hide")
                            .on_click(cx.listener(|view, _, window, cx| {
                                platform::hide(window, cx);
                                view.state.update(cx, |state, cx| {
                                    state.floating_visible = false;
                                    cx.notify();
                                });
                            })),
                    ),
            );
            if reminder {
                if self.picker.as_ref().is_none_or(|(note, _)| *note != id) {
                    let state = self.state.clone();
                    let note = id.clone();
                    let picker = cx.new(|cx| ReminderPicker::new(state, note, true, window, cx));
                    self.picker_subscription = Some(cx.observe(&picker, |_, _, cx| cx.notify()));
                    self.picker = Some((id.clone(), picker));
                }
                if let Some((_, picker)) = &self.picker {
                    content = content.child(div().p_5().child(picker.clone()));
                }
            } else if self.state.read(cx).notes.contains_key(&id) {
                if self.editor.as_ref().is_none_or(|(note, _)| *note != id) {
                    let state = self.state.clone();
                    let note = id.clone();
                    let editor = cx.new(|cx| {
                        let mut editor = NoteEditor::new(state, note, window, cx);
                        editor.compact = true;
                        editor
                    });
                    editor.update(cx, |editor, cx| editor.focus_title(window, cx));
                    self.editor = Some((id, editor));
                }
                if let Some((_, editor)) = &self.editor {
                    content = content.child(div().flex_1().min_h_0().child(editor.clone()));
                }
            }
        } else {
            content = content.child(
                div()
                    .h(px(56.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .px_3()
                    .gap_2()
                    .child(
                        div()
                            .id("island-drag-compact")
                            .cursor_move()
                            .py_2()
                            .child(
                                Icon::new(IconName::NotebookPen)
                                    .size_5()
                                    .text_color(p.accent),
                            )
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|view, _, window, cx| view.start_drag(window, cx)),
                            ),
                    )
                    .child(
                        div()
                            .relative()
                            .flex_1()
                            .child(
                                Input::new(&self.input)
                                    .appearance(false)
                                    .bordered(false)
                                    .focus_bordered(false)
                                    .w_full()
                                    .text_color(p.text)
                                    .aria_label("Search or write"),
                            )
                            .when(self.input.read(cx).value().is_empty(), |view| {
                                let input = self.input.clone();
                                view.child(
                                    div()
                                        .absolute()
                                        .left(px(8.))
                                        .top_0()
                                        .h_full()
                                        .flex()
                                        .items_center()
                                        .text_color(p.muted)
                                        .text_size(px(14.))
                                        .child("Search or write...")
                                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                            input.update(cx, |input, cx| input.focus(window, cx))
                                        }),
                                )
                            }),
                    )
                    .child(
                        Button::new("island-expand")
                            .ghost()
                            .small()
                            .text_color(p.text)
                            .icon(IconName::ChevronDown)
                            .tooltip("Recent and pinned notes")
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.expanded = !view.expanded;
                                view.search(cx);
                                view.input.update(cx, |input, cx| input.focus(window, cx));
                            })),
                    )
                    .child(
                        Button::new("island-new")
                            .ghost()
                            .small()
                            .text_color(p.text)
                            .icon(IconName::Plus)
                            .tooltip("New quick note")
                            .on_click(cx.listener(|view, _, _, cx| view.quick_note(cx))),
                    ),
            );
            if self.expanded {
                content = content.child(div().h(px(1.)).bg(p.line));
                for (index, note) in self.results.iter().enumerate() {
                    let id = note.id.clone();
                    content = content.child(
                        div()
                            .id(SharedString::from(format!("result-{}", note.id)))
                            .h(px(64.))
                            .flex_shrink_0()
                            .px_5()
                            .py_2()
                            .v_flex()
                            .gap_1()
                            .cursor_pointer()
                            .when(index == self.selected, |row| row.bg(p.selected))
                            .hover(|row| row.bg(p.selected))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .flex_1()
                                            .truncate()
                                            .text_sm()
                                            .font_semibold()
                                            .child(note.title.clone()),
                                    )
                                    .when(note.is_pinned, |row| {
                                        row.child(Icon::new(IconName::Pin).size_3())
                                    })
                                    .when(note.reminder_at.is_some(), |row| {
                                        row.child(
                                            Icon::new(IconName::Bell).size_3().text_color(p.accent),
                                        )
                                    }),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_xs()
                                    .text_color(p.muted)
                                    .child(note.preview.lines().next().unwrap_or("").to_owned()),
                            )
                            .on_click(cx.listener(move |view, _, _, cx| view.open(id.clone(), cx))),
                    );
                }
                if self.results.is_empty() {
                    content = content.child(
                        div()
                            .h(px(64.))
                            .px_5()
                            .py_4()
                            .text_color(p.muted)
                            .text_sm()
                            .child("Press Enter to keep this as a note."),
                    );
                }
                content = content.child(
                    div()
                        .h(px(48.))
                        .px_4()
                        .flex()
                        .items_center()
                        .justify_end()
                        .child(
                            Button::new("island-compact")
                                .ghost()
                                .small()
                                .text_color(p.text)
                                .label("Collapse")
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.expanded = false;
                                    cx.notify();
                                })),
                        ),
                );
            }
        }
        div()
            .id("floating-island")
            .size_full()
            .rounded(px(crate::theme::island_radius(height)))
            .overflow_hidden()
            .bg(p.paper)
            .text_color(p.text)
            .font_family("Segoe UI")
            .text_size(px(14.))
            .child(content)
    }
}

pub fn show(state: Entity<AppState>, visible: bool, cx: &mut App) {
    if !state.read(cx).settings.floating_enabled && visible {
        state.update(cx, |state, cx| {
            state.toast("Enable the floating bar in Settings", cx)
        });
        return;
    }
    let handle = state.read(cx).floating_window;
    if let Some(handle) = handle {
        if let Some(view) = state.read(cx).floating_view.clone() {
            view.update(cx, |view, cx| {
                view.anchor = platform::work_area();
                view.position_override = None;
                view.last_size = (0, 0);
                view.search(cx);
            });
        }
        let _ = handle.update(cx, |_, window, cx| {
            if visible {
                platform::show(window, cx)
            } else {
                platform::hide(window, cx)
            }
        });
        state.update(cx, |state, cx| {
            state.floating_visible = visible;
            cx.notify();
        });
        return;
    }
    if !visible {
        return;
    }
    let settings = state.read(cx).settings.clone();
    let anchor = platform::work_area();
    let options = WindowOptions {
        show: false,
        titlebar: None,
        kind: WindowKind::PopUp,
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            point(px(anchor.0 as f32), px(anchor.1 as f32)),
            size(px(380.), px(56.)),
        ))),
        is_resizable: false,
        is_minimizable: false,
        app_id: Some(platform::APP_ID.into()),
        ..Default::default()
    };
    match gpui_kit::open_window(options, cx, |window, cx| {
        cx.new(|cx| FloatingWindow::new(state.clone(), window, cx))
    }) {
        Ok((handle, view)) => {
            state.update(cx, |state, cx| {
                state.floating_window = Some(handle);
                state.floating_view = Some(view);
                state.floating_visible = true;
                cx.notify();
            });
            let _ = handle.update(cx, |_, window, cx| {
                platform::floating_style(window, &settings, cx);
                platform::show(window, cx);
            });
        }
        Err(error) => state.update(cx, |state, cx| {
            state.fail(format!("Couldn't open the floating bar: {error}"), cx)
        }),
    }
}
