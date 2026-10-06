CREATE TABLE notes (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL DEFAULT '',
    content TEXT NOT NULL DEFAULT '',
    note_type TEXT NOT NULL DEFAULT '"normal"',
    is_pinned INTEGER NOT NULL DEFAULT 0 CHECK(is_pinned IN (0,1)),
    is_archived INTEGER NOT NULL DEFAULT 0 CHECK(is_archived IN (0,1)),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    archived_at INTEGER
);
CREATE INDEX notes_active ON notes(is_archived,is_pinned DESC,updated_at DESC);
CREATE TABLE reminders (
    id TEXT PRIMARY KEY,
    note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    scheduled_at INTEGER NOT NULL,
    recurrence_rule TEXT NOT NULL DEFAULT '"Never"',
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','notified','completed')),
    created_at INTEGER NOT NULL,
    completed_at INTEGER
);
CREATE INDEX reminders_due ON reminders(status,scheduled_at);
CREATE INDEX reminders_note ON reminders(note_id,status);
CREATE TABLE settings (key TEXT PRIMARY KEY,value TEXT NOT NULL);
