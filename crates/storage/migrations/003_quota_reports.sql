-- Migration v003: Quota reports and windows (replaces primitive quota_snapshots)

CREATE TABLE IF NOT EXISTS quota_reports (
    provider_id          TEXT NOT NULL PRIMARY KEY,
    account_fingerprint  TEXT NOT NULL DEFAULT '',
    plan                 TEXT NOT NULL DEFAULT '',
    status               TEXT NOT NULL,
    source               TEXT NOT NULL DEFAULT '',
    collected_at         TEXT NOT NULL,
    stale_at             TEXT NOT NULL,
    error_code           TEXT
);

CREATE TABLE IF NOT EXISTS quota_windows (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    provider_id       TEXT NOT NULL,
    window_key        TEXT NOT NULL,
    label             TEXT NOT NULL DEFAULT '',
    scope             TEXT NOT NULL DEFAULT '',
    kind              TEXT NOT NULL DEFAULT '',
    used              INTEGER NOT NULL DEFAULT 0,
    quota_limit       INTEGER NOT NULL DEFAULT 0,
    remaining         INTEGER NOT NULL DEFAULT 0,
    used_percent      REAL NOT NULL DEFAULT 0,
    remaining_percent REAL NOT NULL DEFAULT 0,
    reset_at          TEXT,
    is_unlimited      INTEGER NOT NULL DEFAULT 0,
    confidence        TEXT NOT NULL DEFAULT 'Medium',
    collected_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_quota_windows_provider_time
    ON quota_windows(provider_id, collected_at DESC);
