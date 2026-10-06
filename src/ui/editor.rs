use crate::{app::AppState, models::date_label};
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    button::*,
    dialog::DialogButtonProps,
    input::{Input, InputEvent, InputState, Textarea, TextareaState},
    menu::*,
    *,
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

pub struct NoteEditor {
    pub state: Entity<AppState>,
    pub id: String,
    pub title: Entity<InputState>,
    pub body: Entity<TextareaState>,
    pub compact: bool,
    reading: bool,
    _subscriptions: Vec<Subscription>,
}

impl NoteEditor {
    pub fn new(
        state: Entity<AppState>,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let title = cx.new(|cx| InputState::new(window, cx).placeholder("Untitled"));
        let body = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("Start writing...")
                .searchable(true)
        });
        let mut view = Self {
            state: state.clone(),
            id,
            title: title.clone(),
            body: body.clone(),
            compact: false,
            reading: false,
            _subscriptions: Vec::new(),
        };
        view.sync(window, cx);
        view._subscriptions
            .push(cx.observe_in(&state, window, |view, _, window, cx| {
                view.sync(window, cx);
                cx.notify();
            }));
        view._subscriptions
            .push(cx.subscribe_in(&title, window, |view, _, event, _, cx| {
                if matches!(event, InputEvent::Change) {
                    view.changed(cx);
                }
            }));
        view._subscriptions
            .push(cx.subscribe_in(&body, window, |view, _, event, _, cx| {
                if matches!(event, InputEvent::Change) {
                    view.changed(cx);
                }
            }));
        view
    }

    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(buffer) = self.state.read(cx).notes.get(&self.id) {
            let title = buffer.note.title.clone();
            let content = buffer.note.content.clone();
            if self.title.read(cx).value().as_ref() != title {
                self.title
                    .update(cx, |input, cx| input.set_value(title, window, cx));
            }
            if self.body.read(cx).value().as_ref() != content {
                self.body
                    .update(cx, |input, cx| input.set_value(content, window, cx));
            }
        }
    }

    fn changed(&self, cx: &mut Context<Self>) {
        let title = self.title.read(cx).value().to_string();
        let body = self.body.read(cx).value().to_string();
        self.state
            .update(cx, |state, cx| state.edit(&self.id, title, body, cx));
    }

    pub fn focus_title(&self, window: &mut Window, cx: &mut App) {
        self.title.update(cx, |input, cx| input.focus(window, cx));
    }
    pub fn find(&self, window: &mut Window, cx: &mut App) {
        self.body.update(cx, |input, cx| {
            input.focus(window, cx);
            input.open_search(false, cx);
        });
    }
}

pub fn confirm_delete(state: Entity<AppState>, id: String, window: &mut Window, cx: &mut App) {
    window.open_dialog(cx, move |dialog, _, _| {
        let state = state.clone();
        let id = id.clone();
        dialog
            .title("Delete this note?")
            .child("This permanently deletes the note and its reminders.")
            .button_props(
                DialogButtonProps::default()
                    .ok_text("Delete note")
                    .ok_variant(ButtonVariant::Danger)
                    .show_cancel(true),
            )
            .on_ok(move |_, _, cx| {
                state.update(cx, |state, cx| state.delete(&id, cx));
                true
            })
    });
}

pub fn note_menu(mut menu: PopupMenu, state: Entity<AppState>, id: String, cx: &App) -> PopupMenu {
    let app = state.read(cx);
    let pinned = app
        .notes
        .get(&id)
        .map(|buffer| buffer.note.is_pinned)
        .or_else(|| {
            app.summaries
                .iter()
                .find(|note| note.id == id)
                .map(|note| note.is_pinned)
        })
        .unwrap_or(false);
    let archived = app
        .notes
        .get(&id)
        .map(|buffer| buffer.note.is_archived)
        .or_else(|| {
            app.summaries
                .iter()
                .find(|note| note.id == id)
                .map(|note| note.is_archived)
        })
        .unwrap_or(false);
    let scratch = app
        .notes
        .get(&id)
        .is_some_and(|buffer| buffer.note.note_type == crate::models::NoteType::Scratch);
    for (label, command) in [
        ("Open", "open"),
        ("Rename", "rename"),
        ("Set reminder", "reminder"),
        (if pinned { "Unpin" } else { "Pin" }, "pin"),
        ("Duplicate", "duplicate"),
        (
            if scratch {
                "Keep as normal note"
            } else {
                "Make scratch note"
            },
            if scratch { "normal" } else { "scratch" },
        ),
        (if archived { "Restore" } else { "Archive" }, "archive"),
    ] {
        let state = state.clone();
        let id = id.clone();
        menu = menu.item(PopupMenuItem::new(label).on_click(move |_, window, cx| {
            if command == "reminder" {
                super::reminders::open_picker(state.clone(), id.clone(), window, cx);
            } else {
                state.update(cx, |state, cx| state.note_action(&id, command, cx));
            }
        }));
    }
    let move_state = state.clone();
    let move_id = id.clone();
    menu = menu
        .separator()
        .item(
            PopupMenuItem::new("Move to category…").on_click(move |_, window, cx| {
                super::categories::move_note(move_state.clone(), move_id.clone(), window, cx);
            }),
        );
    menu.separator().item(
        PopupMenuItem::new("Delete permanently")
            .on_click(move |_, window, cx| confirm_delete(state.clone(), id.clone(), window, cx)),
    )
}

impl Render for NoteEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = crate::theme::surfaces(&self.state.read(cx).settings, cx);
        let app = self.state.read(cx);
        let Some(buffer) = app.notes.get(&self.id) else {
            return div().into_any_element();
        };
        let note = &buffer.note;
        let pinned = note.is_pinned;
        let archived = note.is_archived;
        let dirty = buffer.revision != buffer.saved_revision;
        let save_failed = buffer.save_failed;
        let words = note.content.split_whitespace().count();
        let size = app.settings.editor_font_size;
        let reminder = app
            .reminders
            .iter()
            .find(|r| r.note_id == self.id && r.status != "completed")
            .map(|r| date_label(r.scheduled_at));
        let updated = date_label(note.updated_at);
        let content = note.content.clone();
        let state = self.state.clone();
        let id = self.id.clone();
        let menu_state = self.state.clone();
        let menu_id = self.id.clone();
        let reminder_state = self.state.clone();
        let reminder_id = self.id.clone();
        let reminder_state2 = self.state.clone();
        let caption = if let Some(reminder) = reminder {
            Button::new("attached-reminder")
                .ghost()
                .small()
                .icon(IconName::Bell)
                .label(reminder)
                .text_color(p.accent)
                .on_click(move |_, _, cx| {
                    reminder_state2.update(cx, |state, cx| {
                        state.navigate(crate::models::Collection::Reminders, cx)
                    })
                })
                .into_any_element()
        } else {
            div()
                .flex_1()
                .child(if archived {
                    "Archived"
                } else if note.note_type == crate::models::NoteType::Scratch {
                    "Scratch"
                } else {
                    ""
                })
                .into_any_element()
        };
        let tools = div()
            .flex()
            .items_center()
            .justify_between()
            .h(px(46.))
            .px_6()
            .text_xs()
            .text_color(p.muted)
            .child(caption)
            .child(
                div()
                    .flex()
                    .gap_1()
                    .child(
                        Button::new("read-note")
                            .ghost()
                            .small()
                            .icon(if self.reading {
                                IconName::Pencil
                            } else {
                                IconName::BookOpen
                            })
                            .tooltip(if self.reading {
                                "Edit note"
                            } else {
                                "Reading view"
                            })
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.reading = !view.reading;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("add-reminder")
                            .ghost()
                            .small()
                            .icon(IconName::Bell)
                            .tooltip("Set reminder")
                            .on_click(move |_, window, cx| {
                                super::reminders::open_picker(
                                    reminder_state.clone(),
                                    reminder_id.clone(),
                                    window,
                                    cx,
                                )
                            }),
                    )
                    .child(
                        Button::new("pin")
                            .ghost()
                            .small()
                            .icon(IconName::Pin)
                            .tooltip(if pinned { "Unpin" } else { "Pin note" })
                            .when(pinned, |button| button.text_color(p.accent))
                            .on_click(move |_, _, cx| {
                                state.update(cx, |state, cx| state.toggle_pin(&id, cx))
                            }),
                    )
                    .child(
                        Button::new("note-menu")
                            .ghost()
                            .small()
                            .icon(IconName::Ellipsis)
                            .tooltip("Note actions")
                            .dropdown_menu(move |menu, _, cx| {
                                note_menu(menu, menu_state.clone(), menu_id.clone(), cx)
                            }),
                    ),
            );
        div()
            .size_full()
            .v_flex()
            .min_w_0()
            .bg(p.paper)
            .when(!self.compact, |view| view.child(tools))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .v_flex()
                    .px(px(if self.compact { 24. } else { 48. }))
                    .pt(px(if self.compact { 8. } else { 20. }))
                    .pb_5()
                    .gap_4()
                    .child(
                        Input::new(&self.title)
                            .id("note-title")
                            .appearance(false)
                            .bordered(false)
                            .focus_bordered(false)
                            .h(px(46.))
                            .px_0()
                            .text_size(px(if self.compact { 21. } else { 28. }))
                            .line_height(relative(1.25))
                            .font_weight(FontWeight::SEMIBOLD)
                            .aria_label("Note title"),
                    )
                    .when(!self.reading, |view| {
                        view.child(
                            Textarea::new(&self.body)
                                .appearance(false)
                                .bordered(false)
                                .size_full()
                                .px_0()
                                .text_size(px(size))
                                .line_height(relative(1.65))
                                .aria_label("Note content"),
                        )
                    })
                    .when(self.reading, |view| {
                        view.child(
                            div()
                                .id("reading-view")
                                .flex_1()
                                .min_h_0()
                                .overflow_y_scroll()
                                .text_size(px(size))
                                .line_height(relative(1.65))
                                .child(
                                    gpui_kit::base::text::TextView::markdown(
                                        "note-reading-text",
                                        content,
                                    )
                                    .image_source(|_| ImageSource::from("icons/image-off.svg"))
                                    .on_link_click(|url, _, _, cx| {
                                        if url.starts_with("https://")
                                            || url.starts_with("http://")
                                            || url.starts_with("mailto:")
                                        {
                                            cx.open_url(url);
                                        }
                                    })
                                    .style(
                                        gpui_kit::base::text::TextViewStyle::from_theme(
                                            &gpui_kit::base::Theme::global(cx),
                                        ),
                                    ),
                                ),
                        )
                    }),
            )
            .child(
                div()
                    .h(px(34.))
                    .px_6()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_t_1()
                    .border_color(p.line)
                    .text_xs()
                    .text_color(p.muted)
                    .child(format!("{words} words"))
                    .child(if save_failed {
                        "Not saved".into()
                    } else if dirty {
                        "Saving...".into()
                    } else {
                        format!("Edited {updated}")
                    }),
            )
            .into_any_element()
    }
}
