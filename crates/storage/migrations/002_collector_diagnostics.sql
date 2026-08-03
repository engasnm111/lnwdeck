-- Migration v002: Provider states, collector runs and app settings

CREATE TABLE IF NOT EXISTS provider_states (
    provider_id           TEXT NOT NULL PRIMARY KEY,
    display_name          TEXT NOT NULL DEFAULT '',
    enabled               INTEGER NOT NULL DEFAULT 1,
    detected              INTEGER NOT NULL DEFAULT 0,
    detection_method      TEXT NOT NULL DEFAULT '',
    source_type           TEXT NOT NULL DEFAULT '',
    source_exists         INTEGER NOT NULL DEFAULT 0,
    permission_state      TEXT NOT NULL DEFAULT '',
    adapter_version       TEXT NOT NULL DEFAULT '',
    last_detection_at     TEXT,
    detection_error_code  TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS collector_runs (
    id                       INTEGER PRIMARY KEY AUTOINCREMENT,
    provider_id              TEXT NOT NULL,
    collector_mode           TEXT NOT NULL DEFAULT '',
    started_at               TEXT NOT NULL,
    finished_at              TEXT NOT NULL,
    duration_ms              INTEGER NOT NULL DEFAULT 0,
    source_records_seen      INTEGER NOT NULL DEFAULT 0,
    records_parsed           INTEGER NOT NULL DEFAULT 0,
    events_normalized        INTEGER NOT NULL DEFAULT 0,
    events_rejected          INTEGER NOT NULL DEFAULT 0,
    duplicates_skipped       INTEGER NOT NULL DEFAULT 0,
    events_inserted          INTEGER NOT NULL DEFAULT 0,
    quota_snapshots_inserted INTEGER NOT NULL DEFAULT 0,
    warning_codes            TEXT NOT NULL DEFAULT '',
    error_code               TEXT NOT NULL DEFAULT '',
    next_retry_at            TEXT
);

CREATE INDEX IF NOT EXISTS idx_collector_runs_provider_time
    ON collector_runs(provider_id, started_at DESC);

CREATE TABLE IF NOT EXISTS app_settings (
    key   TEXT NOT NULL PRIMARY KEY,
    value TEXT NOT NULL
);
