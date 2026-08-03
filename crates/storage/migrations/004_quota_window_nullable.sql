-- Migration v004: an unknown quota limit is stored as NULL, not as zero.
--
-- Before this migration quota_windows.quota_limit was NOT NULL DEFAULT 0 and
-- an unknown limit was written as 0 with remaining_percent = 100, which reads
-- as "full quota remaining". Limits, remaining and both percentages are now
-- nullable and existing rows with a zero limit are converted to NULL so no
-- historical row keeps a fabricated percentage.

CREATE TABLE quota_windows_v4 (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    provider_id       TEXT NOT NULL,
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

INSERT INTO quota_windows_v4 (
    id, provider_id, window_key, label, scope, kind, used,
    quota_limit, remaining, used_percent, remaining_percent,
    reset_at, is_unlimited, confidence, collected_at
)
SELECT
    id, provider_id, window_key, label, scope, kind, used,
    CASE WHEN quota_limit > 0 THEN quota_limit         ELSE NULL END,
    CASE WHEN quota_limit > 0 THEN remaining           ELSE NULL END,
    CASE WHEN quota_limit > 0 THEN used_percent        ELSE NULL END,
    CASE WHEN quota_limit > 0 THEN remaining_percent   ELSE NULL END,
    reset_at, is_unlimited, confidence, collected_at
FROM quota_windows;

DROP TABLE quota_windows;

ALTER TABLE quota_windows_v4 RENAME TO quota_windows;

CREATE INDEX IF NOT EXISTS idx_quota_windows_provider_time
    ON quota_windows(provider_id, collected_at DESC);
