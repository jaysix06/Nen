use super::Database;
use crate::models::*;
use std::{path::PathBuf, sync::mpsc, time::Duration};

pub enum Request {
    List(Collection, String),
    Load(String),
    Save(Note),
    Delete(String),
    AddReminder(Reminder),
    Complete(String),
    Snooze(String, u32),
    RemoveReminder(String),
    Reminders,
    Settings(Settings),
    Session(Session),
    Shutdown,
}

pub enum Response {
    Notes(Vec<NoteSummary>),
    Note(Option<Note>),
    Reminders(Vec<Reminder>),
    Ok,
}

#[derive(Debug)]
pub enum StoreEvent {
    ReminderDue(Reminder),
    Error(String),
}

type Reply = async_channel::Sender<Result<Response, String>>;

#[derive(Clone)]
pub struct Store {
    sender: mpsc::Sender<(Request, Reply)>,
}

impl Store {
    pub fn start(
        path: PathBuf,
    ) -> anyhow::Result<(Self, Settings, Session, async_channel::Receiver<StoreEvent>)> {
        let database = Database::open(&path)?;
        let settings = database.get_setting("settings")?;
        let session = database.get_setting("session")?;
        let (sender, receiver) = mpsc::channel::<(Request, Reply)>();
        let (events, event_receiver) = async_channel::unbounded();
        std::thread::Builder::new()
            .name("still-storage".into())
            .spawn(move || {
                let mut database = database;
                loop {
                    let now = chrono::Utc::now().timestamp();
                    match database.due(now) {
                        Ok(reminders) => {
                            for reminder in reminders {
                                // Persist delivery state before publishing, so a restart cannot flood notifications.
                                // Missed/failed notifications remain visible in the Reminders page as notified.
                                if let Err(error) = database.mark_notified(&reminder.id) {
                                    let _ = events.try_send(StoreEvent::Error(error.to_string()));
                                } else {
                                    let _ = events.try_send(StoreEvent::ReminderDue(reminder));
                                }
                            }
                        }
                        Err(error) => {
                            let _ = events.try_send(StoreEvent::Error(error.to_string()));
                        }
                    }
                    // No polling while idle. A note write, reminder change or the next due event wakes the worker.
                    let timeout = match database.next_due() {
                        Ok(Some(next)) => Duration::from_secs(
                            (next - chrono::Utc::now().timestamp()).max(0) as u64,
                        ),
                        Ok(None) => Duration::from_secs(86400 * 365),
                        Err(error) => {
                            let _ = events.try_send(StoreEvent::Error(error.to_string()));
                            Duration::from_secs(60)
                        }
                    };
                    let (request, reply) = match receiver.recv_timeout(timeout) {
                        Ok(message) => message,
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    };
                    if matches!(request, Request::Shutdown) {
                        let result = database
                            .checkpoint()
                            .map(|_| Response::Ok)
                            .map_err(|e| e.to_string());
                        // Release SQLite's file handles before acknowledging the shutdown barrier.
                        drop(database);
                        let _ = reply.try_send(result);
                        return;
                    }
                    let result = process(&mut database, request).map_err(|error| error.to_string());
                    let _ = reply.try_send(result);
                }
            })?;
        Ok((Self { sender }, settings, session, event_receiver))
    }

    pub fn request(&self, request: Request) -> async_channel::Receiver<Result<Response, String>> {
        let (reply, receiver) = async_channel::bounded(1);
        if let Err(error) = self.sender.send((request, reply)) {
            let _ = error.0.1.try_send(Err(
                "Storage is unavailable. Keep this window open and retry.".into(),
            ));
        }
        receiver
    }
}

fn process(db: &mut Database, request: Request) -> anyhow::Result<Response> {
    match request {
        Request::List(collection, query) => {
            return Ok(Response::Notes(db.list(collection, &query)?));
        }
        Request::Load(id) => return Ok(Response::Note(db.note(&id)?)),
        Request::Save(note) => db.save_note(&note)?,
        Request::Delete(id) => db.delete(&id)?,
        Request::AddReminder(reminder) => db.add_reminder(&reminder)?,
        Request::Complete(id) => db.complete_reminder(&id, chrono::Utc::now().timestamp())?,
        Request::Snooze(id, minutes) => db.snooze(&id, minutes)?,
        Request::RemoveReminder(id) => db.remove_reminder(&id)?,
        Request::Reminders => return Ok(Response::Reminders(db.reminders()?)),
        Request::Settings(settings) => db.set_setting("settings", &settings)?,
        Request::Session(session) => db.set_setting("session", &session)?,
        Request::Shutdown => db.checkpoint()?,
    }
    Ok(Response::Ok)
}
