-- Migration v007: preserve provider-reported cache and reasoning breakdowns.

ALTER TABLE usage_events ADD COLUMN tokens_cached INTEGER NOT NULL DEFAULT 0;
ALTER TABLE usage_events ADD COLUMN tokens_cache_write INTEGER NOT NULL DEFAULT 0;
ALTER TABLE usage_events ADD COLUMN tokens_reasoning INTEGER NOT NULL DEFAULT 0;
