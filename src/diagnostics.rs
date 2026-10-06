use anyhow::{Context, Result};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};

struct FileLogger(Mutex<File>);
impl log::Log for FileLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Warn
    }
    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata())
            && let Ok(mut file) = self.0.lock()
        {
            let _ = writeln!(
                file,
                "{} {} {}:{} {}",
                chrono::Utc::now().to_rfc3339(),
                record.level(),
                record.target(),
                record.line().unwrap_or(0),
                record.args()
            );
        }
    }
    fn flush(&self) {
        if let Ok(mut file) = self.0.lock() {
            let _ = file.flush();
        }
    }
}

pub fn data_directory() -> Result<PathBuf> {
    if let Some(path) =
        std::env::var_os("NEN_DATA_DIR").or_else(|| std::env::var_os("STILL_DATA_DIR"))
    {
        let path = PathBuf::from(path);
        std::fs::create_dir_all(&path)?;
        return Ok(path);
    }
    let preferred = directories::ProjectDirs::from("dev", "Nen", "Nen")
        .context("Windows local app-data directory is unavailable")?
        .data_local_dir()
        .to_owned();
    let legacy = directories::ProjectDirs::from("dev", "Still", "Still")
        .context("Windows local app-data directory is unavailable")?
        .data_local_dir()
        .to_owned();
    let directory = existing_data_directory(preferred, &legacy);
    std::fs::create_dir_all(&directory)?;
    Ok(directory)
}

fn existing_data_directory(preferred: PathBuf, legacy: &Path) -> PathBuf {
    // Reuse the live database and its managed image paths; never copy an open SQLite database.
    if !preferred.join("notes.sqlite").exists() && legacy.join("notes.sqlite").exists() {
        legacy.to_owned()
    } else {
        preferred
    }
}

pub fn init(directory: &Path) -> Result<()> {
    let path = directory.join("nen.log");
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > 1_048_576) {
        std::fs::rename(&path, directory.join("nen.previous.log"))?;
    }
    let file = OpenOptions::new().create(true).append(true).open(path)?;
    let logger = Box::leak(Box::new(FileLogger(Mutex::new(file))));
    if log::set_logger(logger).is_ok() {
        log::set_max_level(log::LevelFilter::Warn);
    }
    std::panic::set_hook(Box::new(|info| {
        // Log location only. Panic payloads can contain private editor content.
        log::error!("Unexpected failure at {:?}", info.location());
        log::logger().flush();
    }));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        models::{Note, NoteType, Settings},
        storage::Database,
    };

    #[test]
    fn rename_reuses_existing_notes_and_preferences() {
        let root = std::env::temp_dir().join(format!("nen-rename-{}", uuid::Uuid::new_v4()));
        let legacy = root.join("Still/data");
        let preferred = root.join("Nen/data");
        std::fs::create_dir_all(&legacy).expect("legacy directory");
        let mut note = Note::new(NoteType::Normal);
        note.title = "Existing note".into();
        note.content = "Preserve this note".into();
        let settings = Settings {
            editor_font_size: 22.,
            ..Settings::default()
        };
        {
            let db = Database::open(&legacy.join("notes.sqlite")).expect("old database");
            db.save_note(&note).expect("save note");
            db.set_setting("settings", &settings)
                .expect("save preferences");
        }
        let selected = existing_data_directory(preferred.clone(), &legacy);
        assert_eq!(selected, legacy);
        let db = Database::open(&selected.join("notes.sqlite")).expect("reopen");
        assert_eq!(
            db.note(&note.id).expect("load").expect("note").content,
            note.content
        );
        assert_eq!(
            db.get_setting::<Settings>("settings")
                .expect("settings")
                .editor_font_size,
            22.
        );
        assert!(!preferred.exists());
        drop(db);
        assert!(root.is_absolute() && root.starts_with(std::env::temp_dir()));
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn fresh_install_uses_nen_and_existing_nen_data_takes_precedence() {
        let root = std::env::temp_dir().join(format!("nen-paths-{}", uuid::Uuid::new_v4()));
        let preferred = root.join("Nen/data");
        let legacy = root.join("Still/data");
        assert_eq!(
            existing_data_directory(preferred.clone(), &legacy),
            preferred
        );
        for directory in [&preferred, &legacy] {
            std::fs::create_dir_all(directory).expect("directory");
            drop(Database::open(&directory.join("notes.sqlite")).expect("database"));
        }
        assert_eq!(
            existing_data_directory(preferred.clone(), &legacy),
            preferred
        );
        assert!(root.is_absolute() && root.starts_with(std::env::temp_dir()));
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
