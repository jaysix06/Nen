CREATE TABLE categories (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL COLLATE NOCASE UNIQUE CHECK(length(trim(name)) BETWEEN 1 AND 48),
    created_at INTEGER NOT NULL
);
ALTER TABLE notes ADD COLUMN category_id TEXT REFERENCES categories(id) ON DELETE SET NULL;
CREATE INDEX notes_category_active ON notes(category_id, is_archived, is_pinned, updated_at);
INSERT INTO categories(id,name,created_at) VALUES
    ('personal','Personal',unixepoch()),
    ('work','Work',unixepoch()),
    ('ideas','Ideas',unixepoch());
