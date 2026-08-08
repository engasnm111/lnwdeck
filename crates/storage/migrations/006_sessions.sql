-- Migration v006: Session and project attribution
--
-- Privacy-safe attribution: raw session ids and project folder paths never
-- reach this database. Adapters persist keyed hashes (HMAC-SHA-256 with the
-- machine-local secret) in these columns; the raw identifiers are not stored.
-- User-entered display names live in the meta tables below, stored as
-- user-entered metadata only.

ALTER TABLE usage_events ADD COLUMN session_hash TEXT NOT NULL DEFAULT '';
ALTER TABLE usage_events ADD COLUMN project_hash TEXT NOT NULL DEFAULT '';

CREATE INDEX IF NOT EXISTS idx_usage_session_hash
    ON usage_events(session_hash);

CREATE INDEX IF NOT EXISTS idx_usage_project_hash
    ON usage_events(project_hash);

-- User-entered display names for sessions and projects.
CREATE TABLE IF NOT EXISTS session_meta (
    session_hash TEXT NOT NULL PRIMARY KEY,
    display_name TEXT NOT NULL DEFAULT '',
    updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS project_meta (
    project_hash TEXT NOT NULL PRIMARY KEY,
    display_name TEXT NOT NULL DEFAULT '',
    updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
);
