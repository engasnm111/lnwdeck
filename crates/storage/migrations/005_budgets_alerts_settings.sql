-- Migration v005: real budgets, alerts and background-event records.
--
-- These tables back the Budgets and Alerts pages and the System page's
-- background-error list. They replace static placeholder screens; no row is
-- ever seeded, so an empty table renders as "nothing configured" rather than
-- as a healthy status.

-- Spending and token caps configured by the user.
CREATE TABLE IF NOT EXISTS budgets (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    scope        TEXT    NOT NULL,                -- 'global' | 'provider'
    provider_id  TEXT    NOT NULL DEFAULT '',     -- empty for global scope
    period       TEXT    NOT NULL,                -- 'daily' | 'weekly' | 'monthly'
    cost_limit   TEXT    NOT NULL DEFAULT '',     -- decimal string, empty when unset
    token_limit  INTEGER,                         -- NULL when unset
    warn_percent INTEGER NOT NULL DEFAULT 80,
    enabled      INTEGER NOT NULL DEFAULT 1,
    created_at   TEXT    NOT NULL,
    updated_at   TEXT    NOT NULL,
    UNIQUE(scope, provider_id, period)
);

-- Alerts raised by the evaluator from real quota, collector and budget state.
CREATE TABLE IF NOT EXISTS alerts (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    alert_key       TEXT NOT NULL UNIQUE,         -- dedupe key, e.g. 'quota:opencode:5h'
    kind            TEXT NOT NULL,                -- quota_threshold|collector_error|auth_expired|rate_limited|budget_warning|budget_exceeded
    severity        TEXT NOT NULL,                -- info|warning|critical
    provider_id     TEXT NOT NULL DEFAULT '',
    title           TEXT NOT NULL,
    detail          TEXT NOT NULL DEFAULT '',
    error_code      TEXT NOT NULL DEFAULT '',
    first_seen_at   TEXT NOT NULL,
    last_seen_at    TEXT NOT NULL,
    occurrences     INTEGER NOT NULL DEFAULT 1,
    acknowledged_at TEXT,
    resolved_at     TEXT
);

CREATE INDEX IF NOT EXISTS idx_alerts_open
    ON alerts(resolved_at, last_seen_at DESC);

-- Sanitized records of background failures that would otherwise be dropped
-- (refresh loop, updater check, migrations).
CREATE TABLE IF NOT EXISTS app_events (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    occurred_at TEXT NOT NULL,
    source      TEXT NOT NULL,                    -- refresh_loop|updater|migration|widget
    level       TEXT NOT NULL,                    -- info|warning|error
    code        TEXT NOT NULL,
    detail      TEXT NOT NULL DEFAULT ''
);

CREATE INDEX IF NOT EXISTS idx_app_events_time
    ON app_events(occurred_at DESC);
