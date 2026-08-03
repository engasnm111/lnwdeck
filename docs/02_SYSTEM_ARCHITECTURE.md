# lnwdeck System Architecture

## 1. Architectural style

`lnwdeck` ใช้ Modular monolith ใน Desktop process ร่วมกับ Process isolation สำหรับ Browser Native Host และ Community Adapter

```text
Local tools / Logs / APIs / Web pages
                |
          Provider collectors
                |
       Normalize + Privacy filter
                |
       Ingestion and deduplication
                |
        SQLite event + rollups
                |
     Query services / Alert engine
                |
 Dashboard / Tray / Floating widget
```

## 2. Repository structure

```text
lnwdeck/
├─ apps/
│  ├─ desktop/
│  │  ├─ src/                       React UI
│  │  └─ src-tauri/                 Tauri shell and commands
│  ├─ browser-extension/            Edge/Chrome MV3
│  └─ native-messaging-host/        Small stdio bridge
├─ crates/
│  ├─ domain/                       Core types and invariants
│  ├─ application/                  Use cases
│  ├─ storage/                      SQLite and migrations
│  ├─ security/                     Secrets, redaction, permissions
│  ├─ provider-runtime/             Detection, scheduling, sandbox
│  ├─ hook-manager/                 Preview, backup, install, rollback
│  ├─ pricing/                      Catalog and cost calculation
│  ├─ analytics/                    Rollups, forecast, budgets
│  ├─ windows-integration/          Tray, startup, notifications
│  └─ providers/
│     ├─ claude/
│     ├─ codex/
│     ├─ cursor/
│     ├─ gemini/
│     ├─ copilot/
│     ├─ opencode/
│     ├─ grok/
│     ├─ kiro/
│     ├─ ollama/
│     └─ openrouter/
├─ packages/
│  ├─ contracts/                    Generated TypeScript contracts
│  ├─ ui/                           Shared React components
│  └─ test-fixtures/                Synthetic sanitized fixtures
├─ schemas/
│  ├─ domain/                       JSON Schema
│  ├─ native-messaging/             Request/response schemas
│  └─ adapter/                      Manifest and WIT contracts
├─ docs/
├─ prompts/
└─ .github/
```

## 3. Process boundaries

### Desktop process

รับผิดชอบ:

- Tauri lifecycle
- Windows tray/windows
- Application use cases
- Read model delivery
- Notifications
- Update orchestration

Desktop process ห้าม Parse untrusted community adapter code โดยตรง

### Browser extension

รับผิดชอบ:

- User-granted host permissions
- Extract normalized quota from supported pages
- Display connection and permission status
- Pass data to Service Worker
- Send Native Message

Extension ห้ามส่ง Cookie, Authorization header, DOM snapshot หรือ HTML ทั้งหน้า

### Native messaging host

รับผิดชอบ:

- Length-prefixed JSON stdio protocol
- Origin validation
- Schema validation
- Hand-off to Desktop through authenticated local IPC
- Strict message size limit
- Short-lived process where practical

### Community adapter runtime

- Wasm component sandbox
- No filesystem/network/clock/random/secret capability unless Core grants it
- Quotas for execution time, memory and output size
- Crash isolation
- Permission revocation

## 4. Core modules

### Domain

Key types:

```rust
pub struct UsageEvent {
    pub id: EventId,
    pub provider_id: ProviderId,
    pub tool_id: ToolId,
    pub model_id: ModelId,
    pub occurred_at: DateTime<Utc>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub cache_write_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    pub request_count: u32,
    pub project_alias: Option<ProjectAlias>,
    pub session_hash: Option<SessionHash>,
    pub source: DataSource,
    pub confidence: Confidence,
}
```

Forbidden fields are not represented in Domain type

### Application

Use cases:

- `scan_providers`
- `refresh_provider`
- `refresh_all`
- `preview_hook_change`
- `apply_hook_change`
- `rollback_hook_change`
- `ingest_usage_batch`
- `save_quota_snapshot`
- `query_overview`
- `query_analytics`
- `recalculate_costs`
- `export_metadata`
- `evaluate_alerts`
- `prepare_update`

### Storage

Repositories:

- Provider repository
- Usage event repository
- Quota repository
- Pricing repository
- Rollup repository
- Budget repository
- Alert repository
- Permission repository
- Audit repository
- Sync cursor repository

### Provider runtime

Responsibilities:

- Discover adapter
- Check capability
- Request permissions
- Run detection
- Schedule collectors
- Maintain health
- Apply backoff
- Cancel jobs
- Normalize result
- Invoke Privacy guard
- Submit batch to Application layer

## 5. Data flow

### Local incremental collection

1. File watcher receives path event
2. Runtime maps event to Provider
3. Adapter reads only allowed file
4. Adapter uses persisted cursor: path hash, file identity, offset, mtime
5. Parser emits normalized batch
6. Privacy guard validates field allowlist
7. Deduplicator computes stable event fingerprint
8. Transaction inserts new events
9. Rollup job updates affected buckets
10. Alert engine evaluates changes
11. UI receives invalidation event and refetches read model

### API collection

1. Scheduler checks next eligible time
2. Core retrieves Credential handle
3. Trusted built-in API client performs request
4. Response is validated and normalized
5. Rate-limit metadata updates scheduler
6. Data is stored
7. Secret is dropped from memory as soon as practical

### Browser collection

1. Content script extracts allowed fields
2. Service worker validates provider/page
3. Native host validates origin and schema
4. Desktop validates nonce and timestamp
5. Quota snapshot is saved
6. Browser payload is discarded

## 6. Error model

```rust
pub enum lnwdeckError {
    PermissionDenied,
    SourceUnavailable,
    AuthenticationExpired,
    RateLimited { retry_at: Option<DateTime<Utc>> },
    InvalidProviderData,
    PrivacyViolation,
    StorageFailure,
    MigrationFailure,
    AdapterCrashed,
    UpdateVerificationFailed,
    UnsupportedArchitecture,
}
```

UI maps errors to actionable user messages without exposing raw Secret or Path

## 7. Refresh strategy

- Hook/File watcher: event-driven
- Passive log scan: adaptive interval
- Official API: provider-specific interval
- Browser Helper: on page update, manual refresh and conservative periodic check
- Failure: exponential backoff with jitter
- Idle: slower refresh
- Active session: faster refresh
- Manual refresh: bypass ordinary schedule but respect hard rate limits

## 8. Cross-platform posture

Domain, storage, provider and analytics crates must not depend on Windows APIs. Windows-specific behavior lives in `windows-integration`, Tauri shell, installer and native messaging registration.
