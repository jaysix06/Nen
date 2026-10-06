use crate::models::*;
use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;

pub struct Database {
    pub(super) connection: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        let mut connection = Connection::open(path).context("Could not open notes database")?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;",
        )?;
        let version: u32 = connection.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        anyhow::ensure!(version <= 1, "This database needs a newer version of Still");
        if version < 1 {
            let tx = connection.transaction()?;
            tx.execute_batch(include_str!("schema.sql"))?;
            tx.pragma_update(None, "user_version", 1)?;
            tx.commit()?;
        }
        Ok(Self { connection })
    }

    pub fn save_note(&self, note: &Note) -> Result<()> {
        self.connection.execute(
            "INSERT INTO notes (id,title,content,note_type,is_pinned,is_archived,created_at,updated_at,archived_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)
             ON CONFLICT(id) DO UPDATE SET title=excluded.title,content=excluded.content,note_type=excluded.note_type,
             is_pinned=excluded.is_pinned,is_archived=excluded.is_archived,updated_at=excluded.updated_at,archived_at=excluded.archived_at",
            params![note.id,note.title,note.content,serde_json::to_string(&note.note_type)?,note.is_pinned,note.is_archived,note.created_at,note.updated_at,note.archived_at])?;
        Ok(())
    }

    pub fn note(&self, id: &str) -> Result<Option<Note>> {
        Ok(self.connection.query_row(
            "SELECT id,title,content,note_type,is_pinned,is_archived,created_at,updated_at,archived_at FROM notes WHERE id=?1",
            [id], |row| Ok(Note {
                id:row.get(0)?,title:row.get(1)?,content:row.get(2)?,
                note_type:serde_json::from_str(&row.get::<_,String>(3)?).unwrap_or_default(),
                is_pinned:row.get(4)?,is_archived:row.get(5)?,created_at:row.get(6)?,updated_at:row.get(7)?,archived_at:row.get(8)?,
            })).optional()?)
    }

    pub fn list(&self, collection: Collection, query: &str) -> Result<Vec<NoteSummary>> {
        let query = query.trim().to_lowercase();
        let archived = collection == Collection::Archive;
        let pinned = collection == Collection::Pinned;
        let mut statement = self.connection.prepare(
            "SELECT n.id,CASE WHEN trim(n.title)='' THEN 'Untitled' ELSE n.title END,
              CASE WHEN ?3='' OR instr(lower(n.content),?3)=0 THEN substr(n.content,1,160)
                   ELSE substr(n.content,max(1,instr(lower(n.content),?3)-35),160) END,
              n.updated_at,n.is_pinned,n.is_archived,
              (SELECT min(scheduled_at) FROM reminders WHERE note_id=n.id AND status IN ('pending','notified'))
             FROM notes n WHERE is_archived=?1 AND (?2=0 OR is_pinned=1)
             AND (?3='' OR instr(lower(n.title),?3)>0 OR instr(lower(n.content),?3)>0)
             ORDER BY CASE WHEN ?3!='' AND instr(lower(n.title),?3)>0 THEN 0 ELSE 1 END,is_pinned DESC,updated_at DESC LIMIT 500")?;
        let rows = statement.query_map(params![archived, pinned, query], |row| {
            Ok(NoteSummary {
                id: row.get(0)?,
                title: row.get(1)?,
                preview: row.get(2)?,
                updated_at: row.get(3)?,
                is_pinned: row.get(4)?,
                is_archived: row.get(5)?,
                reminder_at: row.get(6)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        self.connection
            .execute("DELETE FROM notes WHERE id=?1", [id])?;
        Ok(())
    }

    pub fn get_setting<T: serde::de::DeserializeOwned + Default>(&self, key: &str) -> Result<T> {
        let value: Option<String> = self
            .connection
            .query_row("SELECT value FROM settings WHERE key=?1", [key], |row| {
                row.get(0)
            })
            .optional()?;
        match value {
            Some(value) => Ok(serde_json::from_str(&value)?),
            None => Ok(T::default()),
        }
    }

    pub fn set_setting(&self, key: &str, value: &impl serde::Serialize) -> Result<()> {
        self.connection.execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key,serde_json::to_string(value)?])?;
        Ok(())
    }

    pub fn checkpoint(&self) -> Result<()> {
        self.connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")?;
        Ok(())
    }
}
