-- Migration v008: keep usage and quota snapshots isolated by account.
--
-- v003 used provider_id as the quota primary key. Rebuild both quota tables
-- so legacy rows remain the empty/default account while new keyed accounts
-- can coexist. Existing quota windows are joined back to their report before
-- the old tables are replaced.

CREATE TABLE quota_reports_v8 (
    provider_id          TEXT NOT NULL,
    account_fingerprint  TEXT NOT NULL DEFAULT '',
    plan                 TEXT NOT NULL DEFAULT '',
    status               TEXT NOT NULL,
    source               TEXT NOT NULL DEFAULT '',
    collected_at         TEXT NOT NULL,
    stale_at             TEXT NOT NULL,
    error_code           TEXT,
    PRIMARY KEY (provider_id, account_fingerprint)
);

INSERT INTO quota_reports_v8
    (provider_id, account_fingerprint, plan, status, source, collected_at, stale_at, error_code)
SELECT provider_id, account_fingerprint, plan, status, source, collected_at, stale_at, error_code
FROM quota_reports;

CREATE TABLE quota_windows_v8 (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    provider_id       TEXT NOT NULL,
    account_fingerprint TEXT NOT NULL DEFAULT '',
    window_key        TEXT NOT NULL,
    label             TEXT NOT NULL DEFAULT '',
    scope             TEXT NOT NULL DEFAULT '',
    kind              TEXT NOT NULL DEFAULT '',
    used              INTEGER NOT NULL DEFAULT 0,
    quota_limit       INTEGER,
    remaining         INTEGER,
    used_percent      REAL,
    remaining_percent REAL,
    reset_at          TEXT,
    is_unlimited      INTEGER NOT NULL DEFAULT 0,
    confidence        TEXT NOT NULL DEFAULT 'Medium',
    collected_at      TEXT NOT NULL
);

INSERT INTO quota_windows_v8
    (id, provider_id, account_fingerprint, window_key, label, scope, kind, used,
     quota_limit, remaining, used_percent, remaining_percent, reset_at,
     is_unlimited, confidence, collected_at)
SELECT w.id,
       w.provider_id,
       COALESCE(r.account_fingerprint, ''),
       w.window_key,
       w.label,
       w.scope,
       w.kind,
       w.used,
       w.quota_limit,
       w.remaining,
       w.used_percent,
       w.remaining_percent,
       w.reset_at,
       w.is_unlimited,
       w.confidence,
       w.collected_at
FROM quota_windows AS w
LEFT JOIN quota_reports AS r
  ON r.provider_id = w.provider_id
 AND r.collected_at = w.collected_at;

DROP TABLE quota_windows;
DROP TABLE quota_reports;
ALTER TABLE quota_reports_v8 RENAME TO quota_reports;
ALTER TABLE quota_windows_v8 RENAME TO quota_windows;

CREATE INDEX idx_quota_windows_provider_account_time
    ON quota_windows(provider_id, account_fingerprint, collected_at DESC);

ALTER TABLE usage_events ADD COLUMN account_fingerprint TEXT NOT NULL DEFAULT '';

CREATE INDEX idx_usage_provider_account
    ON usage_events(provider_id, account_fingerprint);
