use super::Database;
use crate::models::Category;
use anyhow::Result;
use rusqlite::params;

impl Database {
    pub fn categories(&self) -> Result<Vec<Category>> {
        let mut query = self
            .connection
            .prepare("SELECT id,name FROM categories ORDER BY created_at,rowid")?;
        Ok(query
            .query_map([], |row| {
                Ok(Category {
                    id: row.get(0)?,
                    name: row.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn save_category(&self, id: &str, name: &str) -> Result<()> {
        let name = name.trim();
        anyhow::ensure!(
            (1..=48).contains(&name.chars().count()),
            "Use a category name of 1–48 characters."
        );
        let duplicate: bool = self.connection.query_row("SELECT EXISTS(SELECT 1 FROM categories WHERE still_lower(name)=still_lower(?1) AND id!=?2)", params![name,id], |row| row.get(0))?;
        anyhow::ensure!(!duplicate, "A category with that name already exists.");
        self.connection.execute("INSERT INTO categories(id,name,created_at) VALUES(?1,?2,unixepoch()) ON CONFLICT(id) DO UPDATE SET name=excluded.name", params![id,name])?;
        Ok(())
    }

    pub fn delete_category(&mut self, id: &str) -> Result<()> {
        let tx = self.connection.transaction()?;
        tx.execute("DELETE FROM categories WHERE id=?1", [id])?;
        tx.commit()?;
        Ok(())
    }
}
