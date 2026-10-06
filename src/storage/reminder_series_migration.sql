ALTER TABLE reminders ADD COLUMN series_id TEXT;
UPDATE reminders SET series_id=id;
CREATE INDEX reminders_series ON reminders(series_id);
