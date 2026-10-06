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
        connection.create_scalar_function(
            "still_lower",
            1,
            rusqlite::functions::FunctionFlags::SQLITE_UTF8
                | rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC,
            |context| Ok(context.get::<String>(0)?.to_lowercase()),
        )?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;",
        )?;
        let version: u32 = connection.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        anyhow::ensure!(version <= 4, "This database needs a newer version of Still");
        if version < 1 {
            let tx = connection.transaction()?;
            tx.execute_batch(include_str!("schema.sql"))?;
            tx.pragma_update(None, "user_version", 1)?;
            tx.commit()?;
        }
        if version < 2 {
            let tx = connection.transaction()?;
            tx.execute_batch(include_str!("search_migration.sql"))?;
            tx.pragma_update(None, "user_version", 2)?;
            tx.commit()?;
        }
        if version < 3 {
            let tx = connection.transaction()?;
            tx.execute_batch(include_str!("reminder_series_migration.sql"))?;
            tx.pragma_update(None, "user_version", 3)?;
            tx.commit()?;
        }
        if version < 4 {
            let tx = connection.transaction()?;
            tx.execute_batch(include_str!("category_migration.sql"))?;
            tx.pragma_update(None, "user_version", 4)?;
            tx.commit()?;
        }
        Ok(Self { connection })
    }

    pub fn save_note(&self, note: &Note) -> Result<()> {
        self.connection.execute(
            "INSERT INTO notes (id,title,content,note_type,is_pinned,is_archived,created_at,updated_at,archived_at,category_id)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,(SELECT id FROM categories WHERE id=?10))
             ON CONFLICT(id) DO UPDATE SET title=excluded.title,content=excluded.content,note_type=excluded.note_type,
             is_pinned=excluded.is_pinned,is_archived=excluded.is_archived,updated_at=excluded.updated_at,archived_at=excluded.archived_at,category_id=excluded.category_id",
            params![note.id,note.title,note.content,serde_json::to_string(&note.note_type)?,note.is_pinned,note.is_archived,note.created_at,note.updated_at,note.archived_at,note.category_id])?;
        Ok(())
    }

    pub fn note(&self, id: &str) -> Result<Option<Note>> {
        Ok(self.connection.query_row(
            "SELECT id,title,content,note_type,is_pinned,is_archived,created_at,updated_at,archived_at,category_id FROM notes WHERE id=?1",
            [id], |row| Ok(Note {
                id:row.get(0)?,title:row.get(1)?,content:row.get(2)?,
                note_type:serde_json::from_str(&row.get::<_,String>(3)?).unwrap_or_default(),
                is_pinned:row.get(4)?,is_archived:row.get(5)?,created_at:row.get(6)?,updated_at:row.get(7)?,archived_at:row.get(8)?,
                category_id:row.get(9)?,
            })).optional()?)
    }

    pub fn list(&self, collection: Collection, query: &str) -> Result<Vec<NoteSummary>> {
        self.list_in_category(collection, query, None)
    }

    pub fn list_in_category(
        &self,
        collection: Collection,
        query: &str,
        category: Option<&str>,
    ) -> Result<Vec<NoteSummary>> {
        let query = query.trim().to_lowercase();
        let archived = collection == Collection::Archive;
        let pinned = collection == Collection::Pinned;
        if query.chars().count() >= 3 {
            return self.indexed_search(
                archived,
                pinned,
                &query,
                category,
                collection == Collection::Reminders,
            );
        }
        let mut statement = self.connection.prepare(
            "SELECT n.id,CASE WHEN trim(n.title)='' THEN 'Untitled' ELSE n.title END,
              CASE WHEN ?3='' OR instr(still_lower(n.content),?3)=0 THEN substr(n.content,1,160)
                   ELSE substr(n.content,max(1,instr(still_lower(n.content),?3)-35),160) END,
              n.updated_at,n.is_pinned,n.is_archived,
              (SELECT min(scheduled_at) FROM reminders WHERE note_id=n.id AND status IN ('pending','notified')),n.category_id
             FROM notes n WHERE is_archived=?1 AND (?2=0 OR is_pinned=1)
             AND (?4 IS NULL OR category_id=?4)
             AND (?5=0 OR EXISTS(SELECT 1 FROM reminders WHERE note_id=n.id))
             AND (?3='' OR instr(still_lower(n.title),?3)>0 OR instr(still_lower(n.content),?3)>0)
             ORDER BY CASE WHEN ?3!='' AND instr(still_lower(n.title),?3)>0 THEN 0 ELSE 1 END,is_pinned DESC,updated_at DESC")?;
        let rows = statement.query_map(
            params![
                archived,
                pinned,
                query,
                category,
                collection == Collection::Reminders
            ],
            |row| {
                Ok(NoteSummary {
                    id: row.get(0)?,
                    category_id: row.get(7)?,
                    title: row.get(1)?,
                    preview: row.get(2)?,
                    updated_at: row.get(3)?,
                    is_pinned: row.get(4)?,
                    is_archived: row.get(5)?,
                    reminder_at: row.get(6)?,
                })
            },
        )?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    fn indexed_search(
        &self,
        archived: bool,
        pinned: bool,
        query: &str,
        category: Option<&str>,
        reminders: bool,
    ) -> Result<Vec<NoteSummary>> {
        let query = format!("\"{}\"", query.replace('"', "\"\""));
        let mut statement=self.connection.prepare(
            "SELECT n.id,CASE WHEN trim(n.title)='' THEN 'Untitled' ELSE n.title END,
                snippet(notes_fts,1,'','','…',28),n.updated_at,n.is_pinned,n.is_archived,
                (SELECT min(scheduled_at) FROM reminders WHERE note_id=n.id AND status IN ('pending','notified')),n.category_id
             FROM notes_fts CROSS JOIN notes n ON n.rowid=notes_fts.rowid
             WHERE notes_fts MATCH ?3 AND n.is_archived=?1 AND (?2=0 OR n.is_pinned=1)
                AND (?4 IS NULL OR n.category_id=?4)
                AND (?5=0 OR EXISTS(SELECT 1 FROM reminders WHERE note_id=n.id))
             ORDER BY CASE WHEN n.rowid IN (SELECT rowid FROM notes_fts WHERE title MATCH ?3) THEN 0 ELSE 1 END,
                n.is_pinned DESC,n.updated_at DESC")?;
        let rows = statement.query_map(
            params![archived, pinned, query, category, reminders],
            |row| {
                Ok(NoteSummary {
                    id: row.get(0)?,
                    category_id: row.get(7)?,
                    title: row.get(1)?,
                    preview: row.get(2)?,
                    updated_at: row.get(3)?,
                    is_pinned: row.get(4)?,
                    is_archived: row.get(5)?,
                    reminder_at: row.get(6)?,
                })
            },
        )?;
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
