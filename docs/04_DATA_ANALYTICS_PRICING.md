# inwdeck Data, Analytics and Pricing

## 1. Storage strategy

- SQLite with WAL mode where supported
- Foreign keys enabled
- Transactional migrations
- Automatic backup before migration
- UTC timestamps
- Metadata-only schema
- Event ingestion is append-oriented and idempotent
- Read models use rollups

## 2. Core tables

### `providers`

- `id`
- `display_name`
- `adapter_id`
- `adapter_version`
- `enabled`
- `health_status`
- `last_success_at`
- `last_error_code`
- `next_retry_at`

### `source_instances`

Represents local installation, API account alias or Browser source without storing raw identity

- `id`
- `provider_id`
- `source_type`
- `account_alias`
- `machine_local_hash`
- `created_at`

### `usage_events`

- `id`
- `event_fingerprint` unique
- `provider_id`
- `source_instance_id`
- `tool_id`
- `model_id`
- `occurred_at`
- token category columns
- `request_count`
- `project_alias`
- `session_hash`
- `confidence`
- `data_source`
- `ingested_at`

### `quota_snapshots`

- `id`
- `provider_id`
- `source_instance_id`
- `window_kind`
- `used_ratio`
- `used_amount`
- `limit_amount`
- `unit`
- `resets_at`
- `confidence`
- `collected_at`

### `model_prices`

- `catalog_version`
- `provider_id`
- `model_pattern`
- effective date range
- input/output/cache/reasoning/batch rates
- currency
- source
- confidence

### `cost_records`

- `usage_event_id`
- `actual_minor_units`
- `estimated_minor_units`
- `currency`
- `price_catalog_version`
- `price_source`
- `calculated_at`

### `hourly_rollups` and `daily_rollups`

Aggregates by:

- time bucket
- provider
- tool
- model
- project alias
- confidence

### Supporting tables

- `sync_cursors`
- `budgets`
- `budget_periods`
- `alerts`
- `adapter_permissions`
- `hook_backups`
- `audit_events`
- `settings`
- `schema_metadata`

## 3. Privacy-safe identifiers

- Generate random local secret during first run
- Store secret in Windows Credential Manager
- Hash raw session/project identifiers with HMAC-SHA-256
- Do not persist raw input after alias mapping
- Hash differs across machines by default
- Project alias format: `Project 01`, `Project 02`
- User may rename alias manually; rename is stored as user-entered metadata

## 4. Ingestion

Event fingerprint derives from stable metadata:

```text
provider + source instance + source event id/hash +
occurred_at + model + token counts + request count
```

Rules:

- Same event can be collected repeatedly without duplicate row
- Cursor update and event insert occur in one transaction
- Malformed event does not block valid siblings
- Batch reports rejected row count
- Privacy violation rejects the entire unsafe payload

## 5. Retention

Default:

- Raw usage events: retained indefinitely until user changes policy
- Quota snapshots: high-resolution for 90 days, daily summary afterward
- Diagnostic logs: 14 days
- Hook backups: keep latest 5 per provider
- Update packages: keep current staged package only

User options:

- Keep all
- Keep raw events for 30/90/365 days
- Delete by Provider
- Delete date range
- Delete all local data

Rollups remain only when policy explicitly allows aggregated history after raw deletion

## 6. Analytics

- Total token by category
- Cost actual vs estimated
- Requests and failure count where available
- Provider/model/tool/project alias breakdown
- Heatmap
- Previous-period comparison
- Quota history
- Reset behavior
- Data quality coverage
- Budget progress

## 7. Forecast

v0.1 uses deterministic forecast:

1. Require at least 7 daily data points
2. Compute weighted moving average with higher weight for recent 7 days
3. Adjust partial current day by elapsed fraction
4. Estimate end-of-period cost
5. Display confidence band based on recent daily variance
6. Label result `Estimated`
7. Do not forecast when data coverage is too low

No machine-learning model in v0.1

## 8. Hybrid pricing engine

Priority order:

1. Actual cost supplied by Provider
2. User override
3. Provider-specific catalog
4. General model catalog
5. Unknown

Catalog requirements:

- Bundled offline snapshot
- Signed remote catalog update
- Provider-specific overrides
- Effective date ranges
- Model alias normalization
- Version recorded per cost calculation
- Historical cost is not silently changed
- User-triggered recalculation creates audit record

Money representation:

- Integer minor units when currency scale is known
- Decimal string for rates
- Decimal arithmetic
- No binary floating point

## 9. Export

CSV and JSON export:

- Metadata-only
- User selects date range and dimensions
- Include confidence and price source
- Exclude raw path, prompt, response, code and credentials
- Export pipeline runs privacy validator
- Optional encrypted archive can be added after plain export is stable

## 10. Database health

- `PRAGMA integrity_check` available from Settings
- Backup before migration
- Recovery mode opens read-only when migration fails
- Export recovery bundle excludes Secrets
- Database size and last backup visible in System page
