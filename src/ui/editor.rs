use crate::{app::AppState, models::date_label};
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    button::*,
    dialog::DialogButtonProps,
    input::{Input, InputEvent, InputState, RopeExt, Textarea, TextareaState},
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
    font_size: f32,
    zoom: f32,
    image_busy: bool,
    image_task: Option<Task<()>>,
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
            font_size: state.read(cx).settings.editor_font_size,
            state: state.clone(),
            id,
            title: title.clone(),
            body: body.clone(),
            compact: false,
            reading: cfg!(feature = "ui-testing")
                && std::env::args().any(|arg| arg == "--capture-reading"),
            zoom: 1.,
            image_busy: false,
            image_task: None,
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
        let font_size = self.state.read(cx).settings.editor_font_size;
        if self.font_size != font_size {
            self.font_size = font_size;
            // Invalidate the text entity too, so native cached frames rebuild
            // glyphs, wrapping and the caret when its inherited size changes.
            self.body.update(cx, |_, cx| cx.notify());
        }
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

    fn change_zoom(&mut self, delta: f32, cx: &mut Context<Self>) {
        self.zoom = (self.zoom + delta).clamp(0.5, 3.);
        self.body.update(cx, |_, cx| cx.notify());
        cx.notify();
    }

    fn apply_text_edit(
        &self,
        edit: (std::ops::Range<usize>, String),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.body.update(cx, |input, cx| {
            input.set_disabled(false, cx);
            let text = input.value();
            let cursor = input.cursor();
            let next_cursor = if edit.0.end <= cursor {
                cursor - edit.0.len() + edit.1.len()
            } else {
                edit.0.start + edit.1.len()
            };
            let range = text[..edit.0.start].encode_utf16().count()
                ..text[..edit.0.end].encode_utf16().count();
            input.replace_text_in_range(Some(range), &edit.1, window, cx);
            let position = input.text().offset_to_position(next_cursor);
            input.set_cursor_position(position, window, cx);
        });
    }

    fn toggle_list(
        &mut self,
        kind: super::lists::ListKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.state.read(cx).is_quitting() || self.image_busy {
            return;
        }
        self.reading = false;
        let input = self.body.read(cx);
        let edit = super::lists::toggle(&input.value(), input.selected_range(), kind);
        self.apply_text_edit(edit, window, cx);
        cx.notify();
    }

    fn body_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if !self.body.read(cx).focus_handle(cx).is_focused(window)
            || self.reading
            || self.state.read(cx).is_quitting()
            || self.image_busy
        {
            return;
        }
        let key = &event.keystroke;
        if key.modifiers.control && key.key == "0" {
            self.zoom = 1.;
            self.change_zoom(0., cx);
            cx.stop_propagation();
            return;
        }
        if key.modifiers.control
            || key.modifiers.alt
            || key.modifiers.platform
            || (key.modifiers.shift && key.key != "tab")
        {
            return;
        }
        let key = if key.key == "tab" && key.modifiers.shift {
            "shift-tab"
        } else {
            key.key.as_str()
        };
        self.list_key(key, window, cx);
    }

    fn list_key(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        if !self.body.read(cx).focus_handle(cx).is_focused(window)
            || self.reading
            || self.image_busy
            || self.state.read(cx).is_quitting()
        {
            return;
        }
        if self.body.update(cx, |input, cx| {
            input.marked_text_range(window, cx).is_some()
        }) {
            return;
        }
        let input = self.body.read(cx);
        if let Some(edit) = super::lists::typing(&input.value(), input.selected_range(), key) {
            self.apply_text_edit(edit, window, cx);
            cx.stop_propagation();
        }
    }

    fn insert_image(
        &mut self,
        source: crate::note_images::Source,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.image_busy || self.state.read(cx).is_quitting() {
            return;
        }
        self.image_busy = true;
        self.reading = false;
        let directory = self.state.read(cx).store.directory().to_owned();
        self.image_task = Some(cx.spawn_in(window, async move |view, cx| {
            // Paste callbacks run inside the input entity. Snapshot on the next
            // UI turn so we do not borrow that entity again from its own callback.
            let Ok((text, selection)) = view.update_in(cx, |view, _, cx| {
                let input = view.body.read(cx);
                (input.value().to_string(), input.selected_range())
            }) else { return; };
            let root = directory.clone();
            let result = cx.background_executor().spawn(async move {
                crate::note_images::import(source, &root)
            }).await;
            let _ = view.update_in(cx, |view, window, cx| {
                view.image_busy = false;
                match result {
                    Ok(uri) => {
                        if view.state.read(cx).is_quitting() || view.body.read(cx).value().as_ref() != text || !view.state.read(cx).notes.contains_key(&view.id) {
                            if let Some(path) = crate::note_images::resolve(&uri, &directory) { let _ = std::fs::remove_file(path); }
                            if !view.state.read(cx).is_quitting() {
                                view.state.update(cx, |state, cx| state.fail("The note changed while importing the image. Insert it again.".into(), cx));
                            }
                        } else {
                            let leading = if selection.start == 0 || text[..selection.start].ends_with('\n') { "" } else { "\n" };
                            let trailing = if text[selection.end..].starts_with('\n') { "" } else { "\n" };
                            view.apply_text_edit((selection, format!("{leading}![Image]({uri}){trailing}")), window, cx);
                            view.state.update(cx, |state, cx| state.toast("Image inserted. Open Reading view to see it.", cx));
                        }
                    }
                    Err(error) => view.state.update(cx, |state, cx| state.fail(format!("Couldn't insert this image: {error}"), cx)),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn choose_image(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Insert an image into this note".into()),
        });
        cx.spawn_in(window, async move |view, cx| match paths.await {
            Ok(Ok(Some(paths))) => {
                if let Some(path) = paths.into_iter().next() {
                    let _ = view.update_in(cx, |view, window, cx| {
                        view.insert_image(crate::note_images::Source::File(path), window, cx)
                    });
                }
            }
            Ok(Err(error)) => {
                let _ = view.update_in(cx, |view, _, cx| {
                    view.state.update(cx, |state, cx| {
                        state.fail(format!("Couldn't choose an image: {error}"), cx)
                    })
                });
            }
            _ => {}
        })
        .detach();
    }
}

pub fn confirm_delete(state: Entity<AppState>, id: String, window: &mut Window, cx: &mut App) {
    window.open_alert_dialog(cx, move |dialog, _, _| {
        let state = state.clone();
        let id = id.clone();
        dialog
            .confirm()
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
        let p = if self.compact {
            crate::theme::island_palette(
                self.state.read(cx).wallpaper_color,
                &self.state.read(cx).settings,
                cx,
            )
        } else {
            crate::theme::surfaces(&self.state.read(cx).settings, cx)
        };
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
        let size = self.font_size * self.zoom;
        let image_directory = app.store.directory().to_owned();
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
            .h(px(32.))
            .flex_shrink_0()
            .px_5()
            .text_xs()
            .text_color(p.muted)
            .child(caption)
            .child(
                div()
                    .flex()
                    .gap_1()
                    .child(
                        Button::new("insert-image")
                            .ghost()
                            .small()
                            .icon(IconName::Image)
                            .tooltip("Insert a local image")
                            .disabled(app.is_quitting() || self.image_busy)
                            .on_click(
                                cx.listener(|view, _, window, cx| view.choose_image(window, cx)),
                            ),
                    )
                    .child(
                        Button::new("note-lists")
                            .ghost()
                            .small()
                            .label("Lists")
                            .tooltip("Format selected lines as a list")
                            .disabled(app.is_quitting() || self.image_busy)
                            .dropdown_menu({
                                let editor = cx.entity();
                                move |mut menu, _, _| {
                                    for (label, kind) in [
                                        ("Bulleted list", super::lists::ListKind::Bullet),
                                        ("Numbered list", super::lists::ListKind::Numbered),
                                        ("Checklist", super::lists::ListKind::Checklist),
                                    ] {
                                        let editor = editor.clone();
                                        menu = menu.item(PopupMenuItem::new(label).on_click(
                                            move |_, window, cx| {
                                                editor.update(cx, |view, cx| {
                                                    view.toggle_list(kind, window, cx)
                                                });
                                            },
                                        ));
                                    }
                                    menu
                                }
                            }),
                    )
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
            .capture_key_down(cx.listener(Self::body_key))
            .capture_action(cx.listener(
                |view, action: &gpui_kit::component::input::Enter, window, cx| {
                    if !action.shift {
                        view.list_key("enter", window, cx);
                    }
                },
            ))
            .capture_action(cx.listener(
                |view, _: &gpui_kit::component::input::IndentInline, window, cx| {
                    view.list_key("tab", window, cx)
                },
            ))
            .capture_action(cx.listener(
                |view, _: &gpui_kit::component::input::OutdentInline, window, cx| {
                    view.list_key("shift-tab", window, cx)
                },
            ))
            .size_full()
            .v_flex()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .bg(p.paper)
            .when(!self.compact, |view| view.child(tools))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .v_flex()
                    .px(px(if self.compact { 24. } else { 32. }))
                    .pt(px(if self.compact { 8. } else { 12. }))
                    .pb_5()
                    .gap_2()
                    .child(
                        Input::new(&self.title)
                            .disabled(app.is_quitting())
                            .id("note-title")
                            .appearance(false)
                            .bordered(false)
                            .focus_bordered(false)
                            .h(px(36.))
                            .flex_shrink_0()
                            .px_0()
                            .text_size(px(if self.compact { 20. } else { 22. }))
                            .line_height(relative(1.25))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.text)
                            .aria_label("Note title"),
                    )
                    .child(
                        div()
                            .id("note-body")
                            .test_support()
                            .relative()
                            .flex_1()
                            .min_h_0()
                            .v_flex()
                            .overflow_hidden()
                            .child(
                                canvas(|_, _, _| (), {
                                    let editor = cx.entity();
                                    move |bounds, _, window, _| {
                                        let editor = editor.clone();
                                        window.on_mouse_event(
                                            move |event: &ScrollWheelEvent, phase, _, cx| {
                                                if phase == DispatchPhase::Capture
                                                    && event.modifiers.control
                                                    && bounds.contains(&event.position)
                                                {
                                                    let delta = f32::from(
                                                        event.delta.pixel_delta(px(16.)).y,
                                                    );
                                                    if delta != 0. {
                                                        editor.update(cx, |view, cx| {
                                                            view.change_zoom(
                                                                if delta > 0. { 0.1 } else { -0.1 },
                                                                cx,
                                                            )
                                                        });
                                                    }
                                                    cx.stop_propagation();
                                                }
                                            },
                                        );
                                    }
                                })
                                .absolute()
                                .size_full(),
                            )
                            .when(!self.reading, |view| {
                                view.child(
                                    Textarea::new(&self.body)
                                        .disabled(app.is_quitting() || self.image_busy)
                                        .on_paste({
                                            let editor = cx.entity();
                                            move |item, window, cx| {
                                                if item.entries().iter().any(|entry| matches!(entry, ClipboardEntry::Image(image) if image.bytes().len() as u64 > crate::note_images::MAX_BYTES)) {
                                                    editor.update(cx, |view, cx| view.state.update(cx, |state, cx| state.fail("Choose an image smaller than 20 MB".into(), cx)));
                                                    return true;
                                                }
                                                let source =
                                                    item.entries().iter().find_map(|entry| {
                                                        match entry {
                                                            ClipboardEntry::Image(image) => Some(
                                                                crate::note_images::Source::Bytes(
                                                                    image.bytes().to_vec(),
                                                                ),
                                                            ),
                                                            ClipboardEntry::ExternalPaths(
                                                                paths,
                                                            ) => paths
                                                                .paths()
                                                                .first()
                                                                .cloned()
                                                                .map(
                                                                crate::note_images::Source::File,
                                                            ),
                                                            _ => None,
                                                        }
                                                    });
                                                if let Some(source) = source {
                                                    editor.update(cx, |view, cx| {
                                                        view.insert_image(source, window, cx)
                                                    });
                                                    true
                                                } else {
                                                    false
                                                }
                                            }
                                        })
                                        .appearance(false)
                                        .bordered(false)
                                        .flex_1()
                                        .min_h_0()
                                        .px_0()
                                        .text_size(px(size))
                                        .text_color(p.text)
                                        .line_height(relative(1.55))
                                        .aria_label("Note content"),
                                )
                            })
                            .when(self.reading, |view| {
                                view.child(
                                    div()
                                        .id("reading-view")
                                        .test_support()
                                        .flex_1()
                                        .min_h_0()
                                        .overflow_y_scroll()
                                        .text_size(px(size))
                                        .line_height(relative(1.55))
                                        .child(
                                            gpui_kit::base::text::TextView::markdown(
                                                "note-reading-text",
                                                content,
                                            )
                                            .image_source(move |uri| {
                                                crate::note_images::resolve(
                                                    uri.as_ref(),
                                                    &image_directory,
                                                )
                                                .map(ImageSource::from)
                                                .unwrap_or_else(|| {
                                                    ImageSource::from("icons/image-off.svg")
                                                })
                                            })
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
                    ),
            )
            .child(
                div()
                    .id("note-footer")
                    .test_support()
                    .h(px(34.))
                    .flex_shrink_0()
                    .px_6()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_t_1()
                    .border_color(p.line)
                    .text_xs()
                    .text_color(p.muted)
                    .child(format!("{words} words"))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                Button::new("zoom-out")
                                    .ghost()
                                    .xsmall()
                                    .text_color(p.text)
                                    .icon(IconName::Minus)
                                    .tooltip("Zoom out body text")
                                    .on_click(
                                        cx.listener(|view, _, _, cx| view.change_zoom(-0.1, cx)),
                                    ),
                            )
                            .child(
                                Button::new("zoom-reset")
                                    .ghost()
                                    .xsmall()
                                    .text_color(p.text)
                                    .label(format!("{:.0}%", self.zoom * 100.))
                                    .tooltip("Reset body zoom")
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.zoom = 1.;
                                        view.change_zoom(0., cx);
                                    })),
                            )
                            .child(
                                Button::new("zoom-in")
                                    .ghost()
                                    .xsmall()
                                    .text_color(p.text)
                                    .icon(IconName::Plus)
                                    .tooltip("Zoom in body text")
                                    .on_click(
                                        cx.listener(|view, _, _, cx| view.change_zoom(0.1, cx)),
                                    ),
                            ),
                    )
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
