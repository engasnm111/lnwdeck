-- Migration v001: Initial schema

CREATE TABLE IF NOT EXISTS usage_events (
    id          TEXT NOT NULL PRIMARY KEY,
    batch_id    TEXT NOT NULL,
    timestamp   TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    model       TEXT NOT NULL,
    tokens_input  INTEGER NOT NULL DEFAULT 0,
    tokens_output INTEGER NOT NULL DEFAULT 0,
    confidence    TEXT NOT NULL DEFAULT 'High',
    data_source   TEXT NOT NULL DEFAULT '',
    cost          TEXT NOT NULL DEFAULT '0',
    ingested_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_usage_timestamp
    ON usage_events(timestamp);

CREATE INDEX IF NOT EXISTS idx_usage_provider
    ON usage_events(provider_id);

CREATE INDEX IF NOT EXISTS idx_usage_model
    ON usage_events(model);

CREATE TABLE IF NOT EXISTS quota_snapshots (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    provider_id  TEXT NOT NULL,
    quota_limit  INTEGER NOT NULL DEFAULT 0,
    quota_used   INTEGER NOT NULL DEFAULT 0,
    recorded_at  TEXT NOT NULL,
    ingested_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_quota_provider
    ON quota_snapshots(provider_id);

CREATE TABLE IF NOT EXISTS sync_cursors (
    provider_id  TEXT NOT NULL PRIMARY KEY,
    cursor_value TEXT NOT NULL DEFAULT '',
    updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
);
