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
    if let Some(path) = std::env::var_os("STILL_DATA_DIR") {
        let path = PathBuf::from(path);
        std::fs::create_dir_all(&path)?;
        return Ok(path);
    }
    let directory = directories::ProjectDirs::from("dev", "Still", "Still")
        .context("Windows local app-data directory is unavailable")?
        .data_local_dir()
        .to_owned();
    std::fs::create_dir_all(&directory)?;
    Ok(directory)
}

pub fn init(directory: &Path) -> Result<()> {
    let path = directory.join("still.log");
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > 1_048_576) {
        std::fs::rename(&path, directory.join("still.previous.log"))?;
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
