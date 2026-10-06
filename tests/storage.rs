use chrono::{TimeZone, Utc};
use std::path::Path;
use still::{models::*, storage::Database};

#[test]
fn save_reopen_search_archive_and_delete() -> anyhow::Result<()> {
    let database = Database::open(Path::new(":memory:"))?;
    let mut note = Note::new(NoteType::Normal);
    note.title = "Supplier call".into();
    note.content = "Ask about the delivery on Friday.".into();
    database.save_note(&note)?;
    let saved = database.note(&note.id)?.expect("saved note");
    assert_eq!(saved.content, note.content);
    assert_eq!(database.list(Collection::All, "DELIVERY")?.len(), 1);
    assert_eq!(database.list(Collection::All, "%")?.len(), 0);
    note.is_pinned = true;
    database.save_note(&note)?;
    assert_eq!(database.list(Collection::Pinned, "")?.len(), 1);
    note.is_archived = true;
    database.save_note(&note)?;
    assert!(database.list(Collection::All, "")?.is_empty());
    assert_eq!(database.list(Collection::Archive, "")?.len(), 1);
    note.is_archived = false;
    database.save_note(&note)?;
    assert_eq!(database.list(Collection::All, "")?.len(), 1);
    database.delete(&note.id)?;
    assert!(database.note(&note.id)?.is_none());
    Ok(())
}

#[test]
fn search_ranks_title_before_pinned_content() -> anyhow::Result<()> {
    let db = Database::open(Path::new(":memory:"))?;
    let mut content = Note::new(NoteType::Normal);
    content.title = "Work".into();
    content.content = "supplier".into();
    content.is_pinned = true;
    db.save_note(&content)?;
    let mut title = Note::new(NoteType::Normal);
    title.title = "Supplier".into();
    db.save_note(&title)?;
    assert_eq!(db.list(Collection::All, "supplier")?[0].id, title.id);
    Ok(())
}

#[test]
fn settings_round_trip_and_defaults() -> anyhow::Result<()> {
    let db = Database::open(Path::new(":memory:"))?;
    let mut settings: Settings = db.get_setting("appearance")?;
    assert_eq!(settings.theme, "System");
    settings.theme = "Dark".into();
    db.set_setting("appearance", &settings)?;
    assert_eq!(db.get_setting::<Settings>("appearance")?.theme, "Dark");
    Ok(())
}

#[test]
fn recurrence_skips_missed_events_without_duplicates() {
    let scheduled = Utc
        .with_ymd_and_hms(2026, 10, 1, 9, 0, 0)
        .unwrap()
        .timestamp();
    let now = scheduled + 3 * 86400 + 1;
    assert_eq!(
        Recurrence::Daily.next_after(scheduled, now),
        Some(scheduled + 4 * 86400)
    );
    assert_eq!(
        Recurrence::Weekly.next_after(scheduled, now),
        Some(scheduled + 7 * 86400)
    );
    assert_eq!(Recurrence::Never.next_after(scheduled, now), None);
    assert!(
        Recurrence::EveryDays(0)
            .next_after(scheduled, now)
            .is_some()
    );
}

#[test]
fn reopening_applies_migrations_without_data_loss() -> anyhow::Result<()> {
    let path = std::env::temp_dir().join(format!("still-test-{}.sqlite", uuid::Uuid::new_v4()));
    let note = Note::new(NoteType::Normal);
    {
        let db = Database::open(&path)?;
        db.save_note(&note)?;
        db.checkpoint()?;
    }
    {
        let db = Database::open(&path)?;
        assert!(db.note(&note.id)?.is_some());
    }
    std::fs::remove_file(path)?;
    Ok(())
}

#[test]
fn reminder_completion_is_atomic_and_idempotent() -> anyhow::Result<()> {
    let mut db = Database::open(Path::new(":memory:"))?;
    let note = Note::new(NoteType::Normal);
    db.save_note(&note)?;
    let now = Utc::now().timestamp();
    let reminder = Reminder {
        id: uuid::Uuid::new_v4().to_string(),
        note_id: note.id.clone(),
        scheduled_at: now - 1,
        recurrence: Recurrence::Daily,
        status: "pending".into(),
        title: String::new(),
        preview: String::new(),
        series_id: None,
    };
    db.add_reminder(&reminder)?;
    assert_eq!(db.due(now)?.len(), 1);
    db.complete_reminder(&reminder.id, now)?;
    db.complete_reminder(&reminder.id, now)?;
    let reminders = db.reminders()?;
    assert_eq!(reminders.len(), 2);
    assert_eq!(
        reminders.iter().filter(|r| r.status == "pending").count(),
        1
    );
    assert!(db.next_due()?.is_some_and(|date| date > now));
    db.delete(&note.id)?;
    assert!(db.reminders()?.is_empty());
    Ok(())
}

#[test]
fn worker_shutdown_barrier_flushes_queued_writes() -> anyhow::Result<()> {
    use still::storage::{Request, Store};
    let path = std::env::temp_dir().join(format!("still-worker-{}.sqlite", uuid::Uuid::new_v4()));
    let (store, _, _, _) = Store::start(path.clone())?;
    let mut note = Note::new(NoteType::Normal);
    for index in 0..25 {
        note.content = format!("Revision {index}");
        let _ = store.request(Request::Save(note.clone()));
    }
    assert!(store.request(Request::Shutdown).recv_blocking()?.is_ok());
    drop(store);
    {
        let db = Database::open(&path)?;
        assert_eq!(
            db.note(&note.id)?.expect("saved note").content,
            "Revision 24"
        );
    }
    std::fs::remove_file(path)?;
    Ok(())
}

#[test]
fn recurring_delivery_continues_without_completion_and_snooze_does_not_duplicate()
-> anyhow::Result<()> {
    let mut db = Database::open(Path::new(":memory:"))?;
    let note = Note::new(NoteType::Normal);
    db.save_note(&note)?;
    let reminder = Reminder {
        id: uuid::Uuid::new_v4().to_string(),
        note_id: note.id.clone(),
        scheduled_at: Utc::now().timestamp() - 1,
        recurrence: Recurrence::Daily,
        status: "pending".into(),
        title: String::new(),
        preview: String::new(),
        series_id: None,
    };
    db.add_reminder(&reminder)?;
    db.mark_notified(&reminder.id)?;
    db.mark_notified(&reminder.id)?;
    assert_eq!(db.reminders()?.len(), 2);
    db.snooze(&reminder.id, 10)?;
    db.complete_reminder(&reminder.id, Utc::now().timestamp())?;
    assert_eq!(db.reminders()?.len(), 2);
    Ok(())
}

#[test]
fn monthly_recurrence_keeps_its_original_day() {
    use chrono::{Datelike, Local, TimeZone};
    let january = Local
        .with_ymd_and_hms(2027, 1, 31, 9, 0, 0)
        .single()
        .expect("January")
        .timestamp();
    let february = Recurrence::MonthlyOn(31)
        .next_after(january, january)
        .expect("February");
    let march = Recurrence::MonthlyOn(31)
        .next_after(february, february)
        .expect("March");
    assert_eq!(
        chrono::DateTime::from_timestamp(february, 0)
            .expect("date")
            .with_timezone(&Local)
            .day(),
        28
    );
    assert_eq!(
        chrono::DateTime::from_timestamp(march, 0)
            .expect("date")
            .with_timezone(&Local)
            .day(),
        31
    );
}

#[test]
fn indexed_search_handles_unicode_edits_and_deletion() -> anyhow::Result<()> {
    let db = Database::open(Path::new(":memory:"))?;
    let mut note = Note::new(NoteType::Normal);
    note.title = "CAFÉ on the corner".into();
    note.content = "Meet Élise on Friday".into();
    db.save_note(&note)?;
    assert_eq!(db.list(Collection::All, "café")?.len(), 1);
    assert_eq!(db.list(Collection::All, "élise")?.len(), 1);
    assert_eq!(db.list(Collection::All, "É")?.len(), 1);
    note.content = "Meet Anna on Monday".into();
    db.save_note(&note)?;
    assert!(db.list(Collection::All, "élise")?.is_empty());
    assert_eq!(db.list(Collection::All, "Anna")?.len(), 1);
    db.delete(&note.id)?;
    assert!(db.list(Collection::All, "café")?.is_empty());
    Ok(())
}

#[test]
fn upgrading_a_v1_database_backfills_search_without_changing_notes() -> anyhow::Result<()> {
    let path =
        std::env::temp_dir().join(format!("still-migration-{}.sqlite", uuid::Uuid::new_v4()));
    {
        let connection = rusqlite::Connection::open(&path)?;
        connection.execute_batch(include_str!("../src/storage/schema.sql"))?;
        connection.pragma_update(None, "user_version", 1)?;
        connection.execute("INSERT INTO notes(id,title,content,created_at,updated_at) VALUES('original','Supplier','Private original content',1,1)",[])?;
    }
    {
        let db = Database::open(&path)?;
        assert_eq!(
            db.note("original")?.expect("original note").content,
            "Private original content"
        );
        assert_eq!(db.list(Collection::All, "supplier")?.len(), 1);
    }
    std::fs::remove_file(path)?;
    Ok(())
}

#[test]
fn cancelling_a_repeating_reminder_stops_the_series_and_keeps_the_note() -> anyhow::Result<()> {
    let mut db = Database::open(Path::new(":memory:"))?;
    let note = Note::new(NoteType::Normal);
    db.save_note(&note)?;
    let reminder = Reminder {
        id: uuid::Uuid::new_v4().to_string(),
        note_id: note.id.clone(),
        scheduled_at: Utc::now().timestamp() - 1,
        recurrence: Recurrence::Daily,
        status: "pending".into(),
        title: String::new(),
        preview: String::new(),
        series_id: None,
    };
    db.add_reminder(&reminder)?;
    db.mark_notified(&reminder.id)?;
    let pending = db
        .reminders()?
        .into_iter()
        .find(|r| r.status == "pending")
        .expect("next occurrence");
    db.remove_reminder(&pending.id)?;
    assert!(db.reminders()?.is_empty());
    assert!(db.note(&note.id)?.is_some());
    Ok(())
}
