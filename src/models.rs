use chrono::{DateTime, Datelike, Local, Months, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum NoteType {
    #[default]
    Normal,
    Scratch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    #[serde(default)]
    pub category_id: Option<String>,
    pub title: String,
    pub content: String,
    pub note_type: NoteType,
    pub is_pinned: bool,
    pub is_archived: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived_at: Option<i64>,
}

impl Note {
    pub fn new(note_type: NoteType) -> Self {
        let now = Utc::now().timestamp();
        Self {
            id: Uuid::new_v4().to_string(),
            category_id: None,
            title: String::new(),
            content: String::new(),
            note_type,
            is_pinned: false,
            is_archived: false,
            created_at: now,
            updated_at: now,
            archived_at: None,
        }
    }
    pub fn display_title(&self) -> &str {
        if self.title.trim().is_empty() {
            "Untitled"
        } else {
            &self.title
        }
    }
}

#[derive(Debug, Clone)]
pub struct NoteSummary {
    pub id: String,
    pub category_id: Option<String>,
    pub title: String,
    pub preview: String,
    pub updated_at: i64,
    pub is_pinned: bool,
    pub is_archived: bool,
    pub reminder_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Category {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Collection {
    #[default]
    All,
    Pinned,
    Reminders,
    Archive,
}

impl Collection {
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All notes",
            Self::Pinned => "Pinned",
            Self::Reminders => "Reminders",
            Self::Archive => "Archive",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum Recurrence {
    #[default]
    Never,
    Daily,
    Weekly,
    Monthly,
    MonthlyOn(u32),
    EveryDays(u32),
}

impl Recurrence {
    pub fn next_after(&self, scheduled_at: i64, now: i64) -> Option<i64> {
        if *self == Self::Never {
            return None;
        }
        let mut date = DateTime::from_timestamp(scheduled_at, 0)?.with_timezone(&Local);
        let anchor = if let Self::MonthlyOn(day) = self {
            *day
        } else {
            date.day()
        };
        // Preserve local wall-clock time across daylight-saving transitions.
        for _ in 0..100_000 {
            let local = date.naive_local();
            let next = match self {
                Self::Never => return None,
                Self::Monthly | Self::MonthlyOn(_) => {
                    let first = local.with_day(1)?.checked_add_months(Months::new(1))?;
                    (1..=anchor.min(31))
                        .rev()
                        .find_map(|day| first.with_day(day))?
                }
                Self::Daily => local.checked_add_signed(chrono::Duration::days(1))?,
                Self::Weekly => local.checked_add_signed(chrono::Duration::weeks(1))?,
                Self::EveryDays(days) => {
                    local.checked_add_signed(chrono::Duration::days(i64::from((*days).max(1))))?
                }
            };
            date = Local.from_local_datetime(&next).earliest().or_else(|| {
                Local
                    .from_local_datetime(&(next + chrono::Duration::hours(1)))
                    .earliest()
            })?;
            if date.timestamp() > now {
                return Some(date.timestamp());
            }
        }
        None
    }
    pub fn label(&self) -> String {
        match self {
            Self::Never => "Never".into(),
            Self::Daily => "Daily".into(),
            Self::Weekly => "Weekly".into(),
            Self::Monthly | Self::MonthlyOn(_) => "Monthly".into(),
            Self::EveryDays(days) => format!("Every {days} days"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reminder {
    pub id: String,
    pub note_id: String,
    pub scheduled_at: i64,
    pub recurrence: Recurrence,
    pub status: String,
    pub title: String,
    pub preview: String,
    #[serde(default)]
    pub series_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Session {
    pub tabs: Vec<String>,
    pub active: Option<String>,
    pub width: f32,
    pub height: f32,
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub maximized: bool,
    pub category_id: Option<String>,
    pub sidebar_hidden: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: String,
    pub editor_font_size: f32,
    pub restore_tabs: bool,
    pub default_note_type: NoteType,
    pub minimize_to_tray: bool,
    pub launch_at_startup: bool,
    pub start_in_tray: bool,
    pub floating_enabled: bool,
    pub floating_on_startup: bool,
    pub floating_topmost: bool,
    pub floating_hide_on_blur: bool,
    pub floating_remember_position: bool,
    pub floating_width: f32,
    pub floating_opacity: f32,
    pub floating_position: String,
    pub floating_x: Option<i32>,
    pub floating_y: Option<i32>,
    pub surface: String,
    pub background_color: String,
    pub background_image: Option<String>,
    pub background_fit: String,
    pub background_blur: f32,
    pub background_dim: f32,
    pub background_saturation: f32,
    pub background_opacity: f32,
    pub snooze_minutes: u32,
    pub default_reminder_time: String,
    pub notification_sound: bool,
    pub reduced_motion: bool,
    pub shortcuts: std::collections::BTreeMap<String, String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "System".into(),
            editor_font_size: 17.,
            restore_tabs: true,
            default_note_type: NoteType::Normal,
            minimize_to_tray: true,
            launch_at_startup: false,
            start_in_tray: false,
            floating_enabled: true,
            floating_on_startup: false,
            floating_topmost: true,
            floating_hide_on_blur: true,
            floating_remember_position: true,
            floating_width: 560.,
            floating_opacity: 1.,
            floating_position: "Top center".into(),
            floating_x: None,
            floating_y: None,
            surface: "Opaque".into(),
            background_color: "#f6f5f1".into(),
            background_image: None,
            background_fit: "Cover".into(),
            background_blur: 0.,
            background_dim: 0.25,
            background_saturation: 0.7,
            background_opacity: 0.5,
            snooze_minutes: 10,
            default_reminder_time: "09:00".into(),
            notification_sound: true,
            reduced_motion: false,
            shortcuts: default_shortcuts(),
        }
    }
}

pub fn default_shortcuts() -> std::collections::BTreeMap<String, String> {
    let mut bindings: std::collections::BTreeMap<String, String> = [
        ("new_note", "ctrl-n"),
        ("close_tab", "ctrl-w"),
        ("reopen_tab", "ctrl-shift-t"),
        ("next_tab", "ctrl-tab"),
        ("previous_tab", "ctrl-shift-tab"),
        ("toggle_sidebar", "ctrl-b"),
        ("find", "ctrl-f"),
        ("search", "ctrl-shift-f"),
        ("reminder", "ctrl-r"),
        ("toggle_float", "ctrl-shift-space"),
        ("open_app", "ctrl-alt-n"),
        ("quick_note", "ctrl-alt-space"),
        ("settings", "ctrl-,"),
        ("pin", "ctrl-shift-p"),
        ("archive", "ctrl-shift-a"),
    ]
    .into_iter()
    .map(|(key, value)| (key.into(), value.into()))
    .collect();
    for index in 1..=9 {
        bindings.insert(format!("tab_{index}"), format!("ctrl-{index}"));
    }
    bindings
}

pub fn date_label(timestamp: i64) -> String {
    DateTime::from_timestamp(timestamp, 0)
        .map(|date| {
            date.with_timezone(&Local)
                .format("%b %-d · %-I:%M %p")
                .to_string()
        })
        .unwrap_or_default()
}
