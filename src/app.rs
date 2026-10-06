use crate::{
    models::*,
    storage::{Request, Response, Store},
};
use gpui_kit::*;
use std::{collections::HashMap, time::Duration};

#[derive(Clone)]
pub struct NoteBuffer {
    pub note: Note,
    pub revision: u64,
    pub saved_revision: u64,
}

pub struct AppState {
    pub store: Store,
    pub settings: Settings,
    pub session: Session,
    pub collection: Collection,
    pub query: String,
    pub summaries: Vec<NoteSummary>,
    pub notes: HashMap<String, NoteBuffer>,
    pub reminders: Vec<Reminder>,
    pub closed_tabs: Vec<String>,
    pub message: Option<String>,
    pub error: Option<String>,
    pub settings_page: Option<String>,
    pub floating_note: Option<String>,
    pub floating_visible: bool,
    pub floating_reminder: bool,
    pub focus_title: Option<String>,
    pub main_window: Option<AnyWindowHandle>,
    pub floating_window: Option<AnyWindowHandle>,
    debounce: HashMap<String, Task<()>>,
    message_task: Option<Task<()>>,
    list_generation: u64,
    quitting: bool,
}

impl AppState {
    pub fn new(
        store: Store,
        settings: Settings,
        mut session: Session,
        cx: &mut Context<Self>,
    ) -> Self {
        if !settings.restore_tabs {
            session.tabs.clear();
            session.active = None;
        }
        let tabs = session.tabs.clone();
        let mut state = Self {
            store,
            settings,
            session,
            collection: Collection::All,
            query: String::new(),
            summaries: Vec::new(),
            notes: HashMap::new(),
            reminders: Vec::new(),
            closed_tabs: Vec::new(),
            message: None,
            error: None,
            settings_page: None,
            floating_note: None,
            floating_visible: false,
            floating_reminder: false,
            focus_title: None,
            main_window: None,
            floating_window: None,
            debounce: HashMap::new(),
            message_task: None,
            list_generation: 0,
            quitting: false,
        };
        for id in tabs {
            state.load(&id, cx);
        }
        state.refresh(cx);
        state.refresh_reminders(cx);
        state
    }

    pub fn active_note(&self) -> Option<&NoteBuffer> {
        self.session
            .active
            .as_ref()
            .and_then(|id| self.notes.get(id))
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.list_generation += 1;
        let generation = self.list_generation;
        let response = self
            .store
            .request(Request::List(self.collection, self.query.clone()));
        cx.spawn(async move |state, cx| {
            if let Ok(response) = response.recv().await {
                let _ = state.update(cx, |state, cx| {
                    if state.list_generation != generation {
                        return;
                    }
                    match response {
                        Ok(Response::Notes(notes)) => state.summaries = notes,
                        Err(error) => state.fail(error, cx),
                        _ => {}
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub fn refresh_reminders(&mut self, cx: &mut Context<Self>) {
        let response = self.store.request(Request::Reminders);
        cx.spawn(async move |state, cx| {
            if let Ok(response) = response.recv().await {
                let _ = state.update(cx, |state, cx| {
                    match response {
                        Ok(Response::Reminders(reminders)) => state.reminders = reminders,
                        Err(error) => state.fail(error, cx),
                        _ => {}
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub fn navigate(&mut self, collection: Collection, cx: &mut Context<Self>) {
        self.collection = collection;
        self.settings_page = None;
        self.refresh(cx);
        cx.notify();
    }

    pub fn search(&mut self, query: String, cx: &mut Context<Self>) {
        self.query = query;
        self.refresh(cx);
        cx.notify();
    }

    pub fn load(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.notes.contains_key(id) {
            return;
        }
        let id = id.to_owned();
        let response = self.store.request(Request::Load(id.clone()));
        cx.spawn(async move |state, cx| {
            if let Ok(response) = response.recv().await {
                let _ = state.update(cx, |state, cx| {
                    match response {
                        Ok(Response::Note(Some(note))) => {
                            state.notes.entry(note.id.clone()).or_insert(NoteBuffer {
                                note,
                                revision: 0,
                                saved_revision: 0,
                            });
                        }
                        Ok(Response::Note(None)) => {
                            state.session.tabs.retain(|tab| tab != &id);
                            if state.session.active.as_ref() == Some(&id) {
                                state.session.active = state.session.tabs.first().cloned();
                            }
                        }
                        Err(error) => state.fail(error, cx),
                        _ => {}
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub fn open_note(&mut self, id: &str, cx: &mut Context<Self>) {
        self.settings_page = None;
        if !self.session.tabs.iter().any(|tab| tab == id) {
            self.session.tabs.push(id.into());
        }
        self.session.active = Some(id.into());
        self.load(id, cx);
        self.save_session();
        cx.notify();
    }

    pub fn create_note(&mut self, floating: bool, cx: &mut Context<Self>) -> String {
        let note = Note::new(NoteType::Normal);
        let id = note.id.clone();
        self.notes.insert(
            id.clone(),
            NoteBuffer {
                note,
                revision: 1,
                saved_revision: 0,
            },
        );
        self.collection = Collection::All;
        self.query.clear();
        self.open_note(&id, cx);
        self.focus_title = Some(id.clone());
        if floating {
            self.floating_note = Some(id.clone());
        }
        self.save_now(&id, cx);
        id
    }

    pub fn edit(&mut self, id: &str, title: String, content: String, cx: &mut Context<Self>) {
        if let Some(buffer) = self.notes.get_mut(id) {
            if buffer.note.title == title && buffer.note.content == content {
                return;
            }
            buffer.note.title = title;
            buffer.note.content = content;
            buffer.note.updated_at = chrono::Utc::now().timestamp();
            buffer.revision += 1;
            if let Some(summary) = self.summaries.iter_mut().find(|note| note.id == id) {
                summary.title = buffer.note.display_title().into();
                summary.preview = buffer.note.content.chars().take(160).collect();
                summary.updated_at = buffer.note.updated_at;
            }
            let id = id.to_owned();
            let debounce_id = id.clone();
            let timer = cx.spawn(async move |state, cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(350))
                    .await;
                let _ = state.update(cx, |state, cx| state.save_now(&id, cx));
            });
            self.debounce.insert(debounce_id, timer);
            cx.notify();
        }
    }

    pub fn save_now(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(buffer) = self.notes.get(id) else {
            return;
        };
        if buffer.revision == buffer.saved_revision {
            return;
        }
        let revision = buffer.revision;
        let id = id.to_owned();
        let response = self.store.request(Request::Save(buffer.note.clone()));
        cx.spawn(async move |state, cx| {
            if let Ok(response) = response.recv().await {
                let _ = state.update(cx, |state, cx| {
                    match response {
                        Ok(_) => {
                            if let Some(buffer) = state.notes.get_mut(&id) {
                                buffer.saved_revision = buffer.saved_revision.max(revision);
                            }
                            state.refresh(cx);
                        }
                        Err(error) => state.fail(format!("Couldn't save this note. {error}"), cx),
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub fn close_tab(&mut self, id: &str, cx: &mut Context<Self>) {
        self.save_now(id, cx);
        if let Some(index) = self.session.tabs.iter().position(|tab| tab == id) {
            self.session.tabs.remove(index);
            self.closed_tabs.push(id.into());
            if self.closed_tabs.len() > 20 {
                self.closed_tabs.remove(0);
            }
            if self.session.active.as_deref() == Some(id) {
                self.session.active = self
                    .session
                    .tabs
                    .get(index.min(self.session.tabs.len().saturating_sub(1)))
                    .cloned();
            }
        }
        self.save_session();
        cx.notify();
    }

    pub fn reopen_tab(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.closed_tabs.pop() {
            self.open_note(&id, cx);
        }
    }

    pub fn switch_tab(&mut self, direction: isize, cx: &mut Context<Self>) {
        let len = self.session.tabs.len();
        if len == 0 {
            return;
        }
        let index = self
            .session
            .active
            .as_ref()
            .and_then(|id| self.session.tabs.iter().position(|tab| tab == id))
            .unwrap_or(0);
        let next = (index as isize + direction).rem_euclid(len as isize) as usize;
        let id = self.session.tabs[next].clone();
        self.open_note(&id, cx);
    }

    pub fn toggle_pin(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(buffer) = self.notes.get_mut(id) {
            buffer.note.is_pinned = !buffer.note.is_pinned;
            buffer.revision += 1;
        }
        self.save_now(id, cx);
        cx.notify();
    }

    pub fn note_action(&mut self, id: &str, action: &'static str, cx: &mut Context<Self>) {
        if !self.notes.contains_key(id) {
            let id = id.to_owned();
            let response = self.store.request(Request::Load(id.clone()));
            cx.spawn(async move |state, cx| {
                if let Ok(result) = response.recv().await {
                    let _ = state.update(cx, |state, cx| match result {
                        Ok(Response::Note(Some(note))) => {
                            state.notes.insert(
                                id.clone(),
                                NoteBuffer {
                                    note,
                                    revision: 0,
                                    saved_revision: 0,
                                },
                            );
                            state.note_action(&id, action, cx);
                        }
                        Err(error) => state.fail(error, cx),
                        _ => {}
                    });
                }
            })
            .detach();
            return;
        }
        match action {
            "rename" => {
                self.open_note(id, cx);
                self.focus_title = Some(id.to_owned());
                cx.notify();
            }
            "pin" => self.toggle_pin(id, cx),
            "archive" => self.archive(id, cx),
            "duplicate" => self.duplicate(id, cx),
            _ => self.open_note(id, cx),
        }
    }

    pub fn reorder_tab(&mut self, id: &str, target: &str, cx: &mut Context<Self>) {
        if id == target {
            return;
        }
        if let (Some(from), Some(to)) = (
            self.session.tabs.iter().position(|t| t == id),
            self.session.tabs.iter().position(|t| t == target),
        ) {
            let id = self.session.tabs.remove(from);
            self.session.tabs.insert(to, id);
            self.save_session();
            cx.notify();
        }
    }

    pub fn archive(&mut self, id: &str, cx: &mut Context<Self>) {
        let mut restored = false;
        if let Some(buffer) = self.notes.get_mut(id) {
            buffer.note.is_archived = !buffer.note.is_archived;
            restored = !buffer.note.is_archived;
            buffer.note.archived_at = if restored {
                None
            } else {
                Some(chrono::Utc::now().timestamp())
            };
            buffer.revision += 1;
        }
        self.save_now(id, cx);
        if !restored {
            self.close_tab(id, cx);
        }
        self.toast(
            if restored {
                "Note restored"
            } else {
                "Note archived"
            },
            cx,
        );
    }

    pub fn duplicate(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(buffer) = self.notes.get(id) {
            let mut note = Note::new(buffer.note.note_type.clone());
            note.title = format!("{} copy", buffer.note.display_title());
            note.content = buffer.note.content.clone();
            let id = note.id.clone();
            self.notes.insert(
                id.clone(),
                NoteBuffer {
                    note,
                    revision: 1,
                    saved_revision: 0,
                },
            );
            self.open_note(&id, cx);
            self.save_now(&id, cx);
        }
    }

    pub fn delete(&mut self, id: &str, cx: &mut Context<Self>) {
        // Cancel delayed edits and enqueue deletion after every already-enqueued write.
        self.debounce.remove(id);
        self.notes.remove(id);
        self.session.tabs.retain(|tab| tab != id);
        self.closed_tabs.retain(|tab| tab != id);
        if self.session.active.as_deref() == Some(id) {
            self.session.active = self.session.tabs.first().cloned();
        }
        self.perform(Request::Delete(id.into()), "Note deleted", cx);
        self.save_session();
    }

    pub fn perform(&mut self, request: Request, message: &str, cx: &mut Context<Self>) {
        let response = self.store.request(request);
        let message = message.to_owned();
        cx.spawn(async move |state, cx| {
            if let Ok(response) = response.recv().await {
                let _ = state.update(cx, |state, cx| match response {
                    Ok(_) => {
                        state.refresh(cx);
                        state.refresh_reminders(cx);
                        state.toast(&message, cx);
                    }
                    Err(error) => state.fail(error, cx),
                });
            }
        })
        .detach();
    }

    pub fn save_session(&self) {
        let _ = self.store.request(Request::Session(self.session.clone()));
    }

    pub fn toast(&mut self, message: &str, cx: &mut Context<Self>) {
        self.message = Some(message.into());
        cx.notify();
        self.message_task = Some(cx.spawn(async move |state, cx| {
            cx.background_executor().timer(Duration::from_secs(3)).await;
            let _ = state.update(cx, |state, cx| {
                state.message = None;
                cx.notify();
            });
        }));
    }

    pub fn fail(&mut self, error: String, cx: &mut Context<Self>) {
        log::error!("{error}");
        self.error = Some(error);
        cx.notify();
    }

    pub fn quit(&mut self, cx: &mut Context<Self>) {
        if self.quitting {
            return;
        }
        self.quitting = true;
        self.debounce.clear();
        let mut pending = Vec::new();
        for buffer in self.notes.values() {
            if buffer.revision != buffer.saved_revision {
                pending.push(self.store.request(Request::Save(buffer.note.clone())));
            }
        }
        pending.push(self.store.request(Request::Session(self.session.clone())));
        let store = self.store.clone();
        cx.spawn(async move |state, cx| {
            for response in pending {
                if let Ok(Err(error)) = response.recv().await {
                    let _ = state.update(cx, |state, cx| {
                        state.quitting = false;
                        state.fail(error, cx);
                    });
                    return;
                }
            }
            if let Ok(Ok(_)) = store.request(Request::Shutdown).recv().await {
                cx.update(|cx| cx.quit());
            }
        })
        .detach();
    }
}
