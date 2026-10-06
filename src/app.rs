use crate::{
    models::*,
    storage::{Request, Response, Store},
};
use gpui_kit::*;
use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

#[derive(Clone)]
pub struct NoteBuffer {
    pub note: Note,
    pub revision: u64,
    pub saved_revision: u64,
    pub save_failed: bool,
}

pub struct AppState {
    pub store: Store,
    pub settings: Settings,
    pub session: Session,
    pub collection: Collection,
    pub query: String,
    pub summaries: Vec<NoteSummary>,
    pub categories: Vec<Category>,
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
    pub floating_view: Option<Entity<crate::ui::floating::FloatingWindow>>,
    pub desktop: Option<crate::platform::Desktop>,
    pub shortcut_capture: Option<String>,
    pub background: Option<std::path::PathBuf>,
    background_task: Option<Task<()>>,
    settings_task: Option<Task<()>>,
    session_task: Option<Task<()>>,
    deleting: HashSet<String>,
    deleted_categories: HashSet<String>,
    debounce: HashMap<String, Task<()>>,
    message_task: Option<Task<()>>,
    list_generation: u64,
    category_generation: u64,
    quitting: bool,
}

impl AppState {
    pub fn new(
        store: Store,
        mut settings: Settings,
        mut session: Session,
        cx: &mut Context<Self>,
    ) -> Self {
        if settings.design_revision < 2 {
            // Update the previous untouched appearance defaults while retaining
            // deliberately chosen themes, backgrounds and all other preferences.
            if settings.theme == "System"
                && settings.surface == "Opaque"
                && settings.background_color == "#f6f5f1"
                && settings.background_image.is_none()
            {
                let defaults = Settings::default();
                settings.theme = defaults.theme;
                settings.surface = defaults.surface;
                settings.background_color = defaults.background_color;
                settings.background_blur = defaults.background_blur;
                settings.background_dim = defaults.background_dim;
                settings.background_saturation = defaults.background_saturation;
                settings.background_opacity = defaults.background_opacity;
                settings.default_wallpaper = defaults.default_wallpaper;
                if settings.editor_font_size == 17. {
                    settings.editor_font_size = defaults.editor_font_size;
                }
            } else {
                settings.default_wallpaper = false;
            }
            settings.design_revision = 2;
        }
        for (action, binding) in default_shortcuts() {
            if !settings.shortcuts.contains_key(&action) {
                let available = !settings.shortcuts.values().any(|value| value == &binding);
                settings
                    .shortcuts
                    .insert(action, if available { binding } else { String::new() });
            }
        }
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
            categories: Vec::new(),
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
            floating_view: None,
            desktop: None,
            shortcut_capture: None,
            background: None,
            background_task: None,
            settings_task: None,
            session_task: None,
            deleting: HashSet::new(),
            deleted_categories: HashSet::new(),
            debounce: HashMap::new(),
            message_task: None,
            list_generation: 0,
            category_generation: 0,
            quitting: false,
        };
        for id in tabs {
            state.load(&id, cx);
        }
        state.refresh(cx);
        state.refresh_categories(cx);
        state.refresh_reminders(cx);
        state.prepare_background(cx);
        state
    }

    pub fn active_note(&self) -> Option<&NoteBuffer> {
        self.session
            .active
            .as_ref()
            .and_then(|id| self.notes.get(id))
    }

    pub fn desktop_command(&mut self, command: &str, cx: &mut Context<Self>) {
        match command {
            "quit" => self.quit(cx),
            "toggle_float" => {
                let entity = cx.entity();
                let show = !self.floating_visible;
                cx.defer(move |cx| crate::ui::floating::show(entity, show, cx));
            }
            "show_float" => {
                let entity = cx.entity();
                cx.defer(move |cx| crate::ui::floating::show(entity, true, cx));
            }
            "quick_note" => {
                self.create_note(true, cx);
                let entity = cx.entity();
                cx.defer(move |cx| crate::ui::floating::show(entity, true, cx));
            }
            "settings" | "open_app" => {
                if command == "settings" {
                    self.settings_page = Some("General".into());
                }
                if let Some(handle) = self.main_window {
                    cx.defer(move |cx| {
                        let _ =
                            handle.update(cx, |_, window, cx| crate::platform::show(window, cx));
                    });
                }
                cx.notify();
            }
            _ => {}
        }
    }

    pub fn update_settings(&mut self, settings: Settings, cx: &mut Context<Self>) {
        let appearance_changed = self.settings.theme != settings.theme
            || self.settings.accent_color != settings.accent_color
            || self.settings.reduced_motion != settings.reduced_motion;
        let background_changed = self.settings.background_image != settings.background_image
            || self.settings.default_wallpaper != settings.default_wallpaper
            || self.settings.background_blur != settings.background_blur
            || self.settings.background_saturation != settings.background_saturation
            || self.settings.surface != settings.surface;
        self.settings = settings;
        if appearance_changed {
            crate::theme::apply(&self.settings, cx);
        }
        if background_changed {
            self.prepare_background(cx);
        }
        if let Some(handle) = self.floating_window {
            let settings = self.settings.clone();
            cx.defer(move |cx| {
                let _ = handle.update(cx, |_, window, cx| {
                    crate::platform::floating_style(window, &settings, cx)
                });
            });
        }
        if let Some(view) = self.floating_view.clone() {
            cx.defer(move |cx| view.update(cx, |view, cx| view.invalidate_size(cx)));
        }
        self.settings_task = Some(cx.spawn(async move |state, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(250))
                .await;
            let response = state.update(cx, |state, _| {
                state
                    .store
                    .request(Request::Settings(state.settings.clone()))
            });
            if let Ok(response) = response
                && let Ok(Err(error)) = response.recv().await
            {
                let _ = state.update(cx, |state, cx| state.fail(error, cx));
            }
        }));
        cx.notify();
    }

    pub fn prepare_background(&mut self, cx: &mut Context<Self>) {
        let settings = self.settings.clone();
        self.background_task = Some(cx.spawn(async move |state, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(250))
                .await;
            let result = cx
                .background_executor()
                .spawn(async move {
                    crate::diagnostics::data_directory()
                        .and_then(|directory| crate::background::prepare(&settings, &directory))
                })
                .await;
            let _ = state.update(cx, |state, cx| {
                match result {
                    Ok(path) => {
                        let previous = state.background.take();
                        state.background = path;
                        if let Some(previous) =
                            previous.filter(|previous| Some(previous) != state.background.as_ref())
                        {
                            // Release decoded pixels when a new background replaces this asset.
                            ImageSource::from(previous.clone()).remove_asset(cx);
                            cx.background_executor()
                                .spawn(async move {
                                    let _ = std::fs::remove_file(previous);
                                })
                                .detach();
                        }
                    }
                    Err(error) => state.fail(format!("Couldn't load the background: {error}"), cx),
                }
                cx.notify();
            });
        }));
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.list_generation += 1;
        let generation = self.list_generation;
        let response = self.store.request(Request::CategoryList(
            self.collection,
            self.query.clone(),
            self.session.category_id.clone(),
        ));
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

    pub fn category_label(&self) -> &str {
        self.session
            .category_id
            .as_ref()
            .and_then(|id| self.categories.iter().find(|c| &c.id == id))
            .map(|c| c.name.as_str())
            .unwrap_or(self.collection.label())
    }

    pub fn refresh_categories(&mut self, cx: &mut Context<Self>) {
        self.category_generation += 1;
        let generation = self.category_generation;
        let response = self.store.request(Request::Categories);
        cx.spawn(async move |state, cx| {
            if let Ok(result) = response.recv().await {
                let _ = state.update(cx, |state, cx| {
                    if state.category_generation != generation {
                        return;
                    }
                    match result {
                        Ok(Response::Categories(categories)) => {
                            state.categories = categories
                                .into_iter()
                                .filter(|c| !state.deleted_categories.contains(&c.id))
                                .collect();
                            if state
                                .session
                                .category_id
                                .as_ref()
                                .is_some_and(|id| !state.categories.iter().any(|c| &c.id == id))
                            {
                                state.session.category_id = None;
                                state.refresh(cx);
                                state.save_session(cx);
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

    pub fn select_category(&mut self, id: &str, cx: &mut Context<Self>) {
        self.navigate(Collection::All, cx);
        self.session.category_id = Some(id.into());
        self.refresh(cx);
        self.save_session(cx);
    }

    pub fn save_category(&mut self, id: String, name: String, cx: &mut Context<Self>) {
        if self.deleted_categories.contains(&id) {
            return;
        }
        self.category_generation += 1;
        let response = self.store.request(Request::SaveCategory(id.clone(), name));
        cx.spawn(async move |state, cx| {
            if let Ok(result) = response.recv().await {
                let _ = state.update(cx, |state, cx| match result {
                    Ok(_) => {
                        state.session.category_id = Some(id);
                        state.collection = Collection::All;
                        state.refresh_categories(cx);
                        state.refresh(cx);
                        state.save_session(cx);
                        cx.notify();
                    }
                    Err(error) => state.fail(error, cx),
                });
            }
        })
        .detach();
    }

    pub fn delete_category(&mut self, id: String, cx: &mut Context<Self>) {
        self.category_generation += 1;
        let response = self.store.request(Request::DeleteCategory(id.clone()));
        cx.spawn(async move |state, cx| {
            if let Ok(result) = response.recv().await {
                let _ = state.update(cx, |state, cx| match result {
                    Ok(_) => {
                        state.deleted_categories.insert(id.clone());
                        state.categories.retain(|category| category.id != id);
                        for buffer in state.notes.values_mut() {
                            if buffer.note.category_id.as_ref() == Some(&id) {
                                // SQLite already removed the association; preserve dirty text.
                                buffer.note.category_id = None;
                            }
                        }
                        if state.session.category_id.as_ref() == Some(&id) {
                            state.navigate(Collection::All, cx);
                        } else {
                            state.refresh(cx);
                        }
                        state.toast("Category deleted", cx);
                    }
                    Err(error) => state.fail(error, cx),
                });
            }
        })
        .detach();
    }

    pub fn move_to_category(&mut self, id: &str, category: Option<String>, cx: &mut Context<Self>) {
        if self.deleting.contains(id)
            || category
                .as_ref()
                .is_some_and(|id| self.deleted_categories.contains(id))
        {
            return;
        }
        if let Some(buffer) = self.notes.get_mut(id) {
            buffer.note.category_id = category;
            buffer.revision += 1;
            self.save_now(id, cx);
            cx.notify();
        } else {
            let id = id.to_owned();
            let response = self.store.request(Request::Load(id.clone()));
            cx.spawn(async move |state, cx| {
                if let Ok(Ok(Response::Note(Some(note)))) = response.recv().await {
                    let _ = state.update(cx, |state, cx| {
                        if state.deleting.contains(&id) {
                            return;
                        }
                        state.notes.entry(id.clone()).or_insert(NoteBuffer {
                            note,
                            revision: 0,
                            saved_revision: 0,
                            save_failed: false,
                        });
                        state.move_to_category(&id, category, cx);
                    });
                }
            })
            .detach();
        }
    }

    pub fn navigate(&mut self, collection: Collection, cx: &mut Context<Self>) {
        self.collection = collection;
        self.session.category_id = None;
        self.settings_page = None;
        self.shortcut_capture = None;
        self.query.clear();
        self.refresh(cx);
        self.save_session(cx);
        cx.notify();
    }

    pub fn search(&mut self, query: String, cx: &mut Context<Self>) {
        self.query = query;
        self.refresh(cx);
        cx.notify();
    }

    pub fn load(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.notes.contains_key(id) || self.deleting.contains(id) {
            return;
        }
        let id = id.to_owned();
        let response = self.store.request(Request::Load(id.clone()));
        cx.spawn(async move |state, cx| {
            if let Ok(response) = response.recv().await {
                let _ = state.update(cx, |state, cx| {
                    if state.deleting.contains(&id) {
                        return;
                    }
                    match response {
                        Ok(Response::Note(Some(mut note))) => {
                            if note
                                .category_id
                                .as_ref()
                                .is_some_and(|id| state.deleted_categories.contains(id))
                            {
                                note.category_id = None;
                            }
                            state.notes.entry(note.id.clone()).or_insert(NoteBuffer {
                                note,
                                revision: 0,
                                saved_revision: 0,
                                save_failed: false,
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
        if self.deleting.contains(id) {
            return;
        }
        self.settings_page = None;
        self.shortcut_capture = None;
        if self.collection == Collection::Reminders {
            self.collection = Collection::All;
            self.refresh(cx);
        }
        if !self.session.tabs.iter().any(|tab| tab == id) {
            self.session.tabs.push(id.into());
        }
        self.session.active = Some(id.into());
        self.load(id, cx);
        self.save_session(cx);
        cx.notify();
    }

    pub fn create_note(&mut self, floating: bool, cx: &mut Context<Self>) -> String {
        let mut note = Note::new(self.settings.default_note_type.clone());
        if !floating {
            note.category_id = self.session.category_id.clone();
        }
        let id = note.id.clone();
        self.notes.insert(
            id.clone(),
            NoteBuffer {
                note,
                revision: 1,
                saved_revision: 0,
                save_failed: false,
            },
        );
        self.collection = Collection::All;
        self.query.clear();
        self.open_note(&id, cx);
        self.focus_title = Some(id.clone());
        if floating {
            self.floating_note = Some(id.clone());
        }
        id
    }

    pub fn edit(&mut self, id: &str, title: String, content: String, cx: &mut Context<Self>) {
        if self.deleting.contains(id) {
            return;
        }
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
        if self.deleting.contains(id) {
            return;
        }
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
            let response = response
                .recv()
                .await
                .unwrap_or_else(|_| Err("Storage is unavailable.".into()));
            let _ = state.update(cx, |state, cx| {
                match response {
                    Ok(_) => {
                        if let Some(buffer) = state.notes.get_mut(&id) {
                            buffer.saved_revision = buffer.saved_revision.max(revision);
                            buffer.save_failed = false;
                        }
                        state.prune_cache();
                        state.refresh(cx);
                    }
                    Err(error) => {
                        if let Some(buffer) = state.notes.get_mut(&id) {
                            buffer.save_failed = true;
                        }
                        state.fail(format!("Couldn't save this note. {error}"), cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn close_tab(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.notes.get(id).is_some_and(|buffer| {
            buffer.note.title.trim().is_empty()
                && buffer.note.content.trim().is_empty()
                && !buffer.note.is_pinned
                && !buffer.note.is_archived
        }) && !self.reminders.iter().any(|reminder| reminder.note_id == id)
        {
            self.remove_note(id, false, cx);
            return;
        }
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
        self.save_session(cx);
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
        if self.deleting.contains(id) {
            return;
        }
        if !self.notes.contains_key(id) {
            let id = id.to_owned();
            let response = self.store.request(Request::Load(id.clone()));
            cx.spawn(async move |state, cx| {
                if let Ok(result) = response.recv().await {
                    let _ = state.update(cx, |state, cx| match result {
                        _ if state.deleting.contains(&id) => {}
                        Ok(Response::Note(Some(mut note))) => {
                            if note
                                .category_id
                                .as_ref()
                                .is_some_and(|id| state.deleted_categories.contains(id))
                            {
                                note.category_id = None;
                            }
                            state.notes.insert(
                                id.clone(),
                                NoteBuffer {
                                    note,
                                    revision: 0,
                                    saved_revision: 0,
                                    save_failed: false,
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
            "normal" | "scratch" => {
                if let Some(buffer) = self.notes.get_mut(id) {
                    buffer.note.note_type = if action == "scratch" {
                        NoteType::Scratch
                    } else {
                        NoteType::Normal
                    };
                    buffer.revision += 1;
                }
                self.save_now(id, cx);
                cx.notify();
            }
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
            self.save_session(cx);
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
                    save_failed: false,
                },
            );
            self.open_note(&id, cx);
            self.save_now(&id, cx);
        }
    }

    pub fn delete(&mut self, id: &str, cx: &mut Context<Self>) {
        self.remove_note(id, true, cx);
    }

    fn remove_note(&mut self, id: &str, announce: bool, cx: &mut Context<Self>) {
        if !self.deleting.insert(id.into()) {
            return;
        }
        self.debounce.remove(id);
        let response = self.store.request(Request::Delete(id.into()));
        let id = id.to_owned();
        cx.spawn(async move |state, cx| {
            let result = response.recv().await;
            let _ = state.update(cx, |state, cx| {
                match result {
                    Ok(Ok(_)) => {
                        state.notes.remove(&id);
                        state.session.tabs.retain(|tab| tab != &id);
                        state.closed_tabs.retain(|tab| tab != &id);
                        if state.session.active.as_ref() == Some(&id) {
                            state.session.active = state.session.tabs.first().cloned();
                        }
                        if state.floating_note.as_ref() == Some(&id) {
                            state.floating_note = None;
                            state.floating_reminder = false;
                        }
                        state.save_session(cx);
                        state.refresh(cx);
                        state.refresh_reminders(cx);
                        if announce {
                            state.toast("Note deleted", cx);
                        }
                    }
                    Ok(Err(error)) => {
                        state.deleting.remove(&id);
                        state.fail(format!("Couldn't delete this note. {error}"), cx);
                    }
                    Err(_) => {
                        state.deleting.remove(&id);
                        state.fail(
                            "Couldn't delete this note. Storage is unavailable.".into(),
                            cx,
                        );
                    }
                }
                cx.notify();
            });
        })
        .detach();
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

    pub fn save_session(&mut self, cx: &mut Context<Self>) {
        self.session_task = Some(cx.spawn(async move |state, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(250))
                .await;
            let response = state.update(cx, |state, _| {
                state.store.request(Request::Session(state.session.clone()))
            });
            if let Ok(response) = response
                && let Ok(Err(error)) = response.recv().await
            {
                let _ = state.update(cx, |state, cx| {
                    state.fail(format!("Couldn't save the session. {error}"), cx)
                });
            }
        }));
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

    pub fn retry_saving(&mut self, cx: &mut Context<Self>) {
        self.error = None;
        let dirty: Vec<_> = self
            .notes
            .iter()
            .filter(|(_, buffer)| buffer.revision != buffer.saved_revision)
            .map(|(id, _)| id.clone())
            .collect();
        for id in dirty {
            self.save_now(&id, cx);
        }
        self.update_settings(self.settings.clone(), cx);
        self.save_session(cx);
        cx.notify();
    }

    fn prune_cache(&mut self) {
        let limit = self.session.tabs.len() + 32;
        if self.notes.len() <= limit {
            return;
        }
        let mut candidates: Vec<_> = self
            .notes
            .iter()
            .filter(|(id, buffer)| {
                buffer.revision == buffer.saved_revision
                    && !self.session.tabs.contains(id)
                    && self.floating_note.as_ref() != Some(*id)
                    && !self.deleting.contains(*id)
            })
            .map(|(id, buffer)| (id.clone(), buffer.note.updated_at))
            .collect();
        candidates.sort_by_key(|(_, time)| *time);
        for (id, _) in candidates
            .into_iter()
            .take(self.notes.len().saturating_sub(limit))
        {
            self.notes.remove(&id);
            self.debounce.remove(&id);
        }
    }

    pub fn quit(&mut self, cx: &mut Context<Self>) {
        if self.quitting {
            return;
        }
        self.quitting = true;
        self.settings_task = None;
        self.session_task = None;
        self.background_task = None;
        self.debounce.clear();
        let mut pending = Vec::new();
        for (id, buffer) in &self.notes {
            if !self.deleting.contains(id) && buffer.revision != buffer.saved_revision {
                pending.push(self.store.request(Request::Save(buffer.note.clone())));
            }
        }
        pending.push(self.store.request(Request::Session(self.session.clone())));
        pending.push(self.store.request(Request::Settings(self.settings.clone())));
        let store = self.store.clone();
        cx.spawn(async move |state, cx| {
            for response in pending {
                if let Err(error) = response
                    .recv()
                    .await
                    .unwrap_or_else(|_| Err("Storage is unavailable.".into()))
                {
                    let _ = state.update(cx, |state, cx| {
                        state.quitting = false;
                        state.fail(error, cx);
                    });
                    return;
                }
            }
            match store.request(Request::Shutdown).recv().await {
                Ok(Ok(_)) => cx.update(|cx| cx.quit()),
                result => {
                    let error = match result {
                        Ok(Err(error)) => error,
                        _ => "Storage is unavailable.".into(),
                    };
                    let _ = state.update(cx, |state, cx| {
                        state.quitting = false;
                        state.fail(error, cx);
                    });
                }
            }
        })
        .detach();
    }
}
