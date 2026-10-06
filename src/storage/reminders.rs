use super::Database;
use crate::models::*;
use anyhow::Result;
use rusqlite::params;

impl Database {
    pub fn reminders(&self) -> Result<Vec<Reminder>> {
        self.query_reminders(false, 0)
    }

    fn query_reminders(&self, due_only: bool, now: i64) -> Result<Vec<Reminder>> {
        let mut statement = self.connection.prepare(
            "SELECT r.id,r.note_id,r.scheduled_at,r.recurrence_rule,r.status,
             CASE WHEN trim(n.title)='' THEN 'Untitled' ELSE n.title END,substr(n.content,1,180)
             FROM reminders r JOIN notes n ON n.id=r.note_id
             WHERE ?1=0 OR (r.status='pending' AND r.scheduled_at<=?2) ORDER BY r.scheduled_at",
        )?;
        let rows = statement.query_map(params![due_only, now], |row| {
            Ok(Reminder {
                id: row.get(0)?,
                note_id: row.get(1)?,
                scheduled_at: row.get(2)?,
                recurrence: serde_json::from_str(&row.get::<_, String>(3)?).unwrap_or_default(),
                status: row.get(4)?,
                title: row.get(5)?,
                preview: row.get(6)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn add_reminder(&self, reminder: &Reminder) -> Result<()> {
        self.connection.execute(
            "INSERT INTO reminders(id,note_id,scheduled_at,recurrence_rule,status,created_at)
             VALUES(?1,?2,?3,?4,'pending',?5)",
            params![
                reminder.id,
                reminder.note_id,
                reminder.scheduled_at,
                serde_json::to_string(&reminder.recurrence)?,
                chrono::Utc::now().timestamp()
            ],
        )?;
        Ok(())
    }

    pub fn next_due(&self) -> Result<Option<i64>> {
        Ok(self.connection.query_row(
            "SELECT min(scheduled_at) FROM reminders WHERE status='pending'",
            [],
            |row| row.get(0),
        )?)
    }

    pub fn due(&self, now: i64) -> Result<Vec<Reminder>> {
        self.query_reminders(true, now)
    }

    pub fn mark_notified(&mut self, id: &str) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        let reminder = self.due(now)?.into_iter().find(|r| r.id == id);
        if let Some(reminder) = reminder {
            let tx = self.connection.transaction()?;
            tx.execute(
                "UPDATE reminders SET status='notified' WHERE id=?1 AND status='pending'",
                [id],
            )?;
            insert_next(&tx, &reminder, now)?;
            tx.commit()?;
        }
        Ok(())
    }

    pub fn snooze(&self, id: &str, minutes: u32) -> Result<()> {
        self.connection.execute(
            "UPDATE reminders SET recurrence_rule=CASE WHEN status='notified' THEN '\"Never\"' ELSE recurrence_rule END,
                status='pending',scheduled_at=?2 WHERE id=?1 AND status!='completed'",
            params![
                id,
                chrono::Utc::now().timestamp() + i64::from(minutes.max(1)) * 60
            ],
        )?;
        Ok(())
    }

    pub fn complete_reminder(&mut self, id: &str, now: i64) -> Result<()> {
        let reminder = self
            .reminders()?
            .into_iter()
            .find(|r| r.id == id && r.status != "completed");
        if let Some(reminder) = reminder {
            let tx = self.connection.transaction()?;
            tx.execute(
                "UPDATE reminders SET status='completed',completed_at=?2 WHERE id=?1",
                params![id, now],
            )?;
            if reminder.status == "pending" {
                insert_next(&tx, &reminder, now)?;
            }
            tx.commit()?;
        }
        Ok(())
    }

    pub fn remove_reminder(&self, id: &str) -> Result<()> {
        self.connection
            .execute("DELETE FROM reminders WHERE id=?1", [id])?;
        Ok(())
    }
}

fn insert_next(tx: &rusqlite::Transaction<'_>, reminder: &Reminder, now: i64) -> Result<()> {
    if let Some(next) = reminder.recurrence.next_after(reminder.scheduled_at, now) {
        tx.execute(
            "INSERT INTO reminders(id,note_id,scheduled_at,recurrence_rule,status,created_at)
            VALUES(?1,?2,?3,?4,'pending',?5)",
            params![
                uuid::Uuid::new_v4().to_string(),
                reminder.note_id,
                next,
                serde_json::to_string(&reminder.recurrence)?,
                now
            ],
        )?;
    }
    Ok(())
}
