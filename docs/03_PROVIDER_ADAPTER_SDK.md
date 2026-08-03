# inwdeck Provider Adapter SDK

## 1. Goals

- เพิ่ม Provider โดยไม่แก้ Core
- Capability ไม่จำเป็นต้องเท่ากัน
- Permission ชัดเจน
- Built-in adapters มีประสิทธิภาพสูง
- Community adapters ทำงานใน Sandbox
- Input ทุกชนิดผ่าน Schema validation
- Output เป็น Metadata-only

## 2. Adapter capabilities

```rust
pub struct AdapterCapabilities {
    pub detection: bool,
    pub usage_events: bool,
    pub quota_snapshots: bool,
    pub cost_records: bool,
    pub reset_time: bool,
    pub local_sessions: bool,
    pub official_api: bool,
    pub browser_helper: bool,
    pub hook_installation: bool,
    pub passive_scan: bool,
}
```

## 3. Built-in adapter trait

```rust
#[async_trait]
pub trait ProviderAdapter: Send + Sync {
    fn descriptor(&self) -> &ProviderDescriptor;

    async fn detect(
        &self,
        ctx: &DetectionContext,
    ) -> Result<DetectionResult, AdapterError>;

    async fn collect_usage(
        &self,
        ctx: &CollectionContext,
        cursor: Option<SyncCursor>,
    ) -> Result<UsageBatch, AdapterError>;

    async fn collect_quota(
        &self,
        ctx: &CollectionContext,
    ) -> Result<Option<QuotaSnapshot>, AdapterError>;

    async fn health(
        &self,
        ctx: &HealthContext,
    ) -> AdapterHealth;
}
```

Hook support is a separate interface so read-only adapters do not receive mutation capability:

```rust
#[async_trait]
pub trait HookCapableAdapter: ProviderAdapter {
    async fn preview_hook(
        &self,
        ctx: &HookContext,
    ) -> Result<HookChangeSet, AdapterError>;

    async fn validate_hook(
        &self,
        ctx: &HookContext,
    ) -> Result<HookValidation, AdapterError>;
}
```

Core `hook-manager` performs Backup, Atomic write and Rollback

## 4. Community adapter manifest

```json
{
  "schemaVersion": 1,
  "id": "community.example-provider",
  "name": "Example Provider",
  "version": "0.1.0",
  "publisher": {
    "name": "Example Maintainer",
    "sourceRepository": "https://example.invalid/example-provider"
  },
  "runtime": {
    "type": "wasm-component",
    "entry": "adapter.wasm",
    "minimumInwdeckVersion": "0.1.0"
  },
  "architectures": ["portable-wasm"],
  "capabilities": [
    "detection",
    "usage-events",
    "quota-snapshots"
  ],
  "permissions": {
    "filesystem": [
      {
        "scope": "user-home-relative",
        "pattern": ".example/sessions/**/*.jsonl",
        "access": "read"
      }
    ],
    "network": [
      {
        "scheme": "https",
        "host": "api.example.invalid",
        "methods": ["GET"]
      }
    ],
    "credentials": [],
    "hooks": []
  }
}
```

Manifest example uses reserved invalid domain intentionally. Real adapter must declare real source repository and domains.

## 5. Permission model

Permission types:

- `filesystem.read`
- `network.request`
- `credential.use`
- `browser.page`
- `hook.preview`
- `hook.install`
- `notification.request`

Rules:

- Default deny
- Scope must be exact
- Wildcard domain forbidden
- Absolute filesystem scope must be approved separately
- Adapter receives data stream or capability handle, not raw unrestricted OS access
- Credential use returns request result where possible, not Secret value
- Permission change requires re-approval
- Permission is revocable
- Unverified adapter displays warning

## 6. Normalized output

### Usage batch

```rust
pub struct UsageBatch {
    pub provider_id: ProviderId,
    pub source_instance: SourceInstanceId,
    pub cursor: Option<SyncCursor>,
    pub events: Vec<UsageEvent>,
    pub collected_at: DateTime<Utc>,
    pub warnings: Vec<CollectionWarning>,
}
```

### Quota snapshot

```rust
pub struct QuotaSnapshot {
    pub provider_id: ProviderId,
    pub account_alias: AccountAlias,
    pub window: QuotaWindow,
    pub used_ratio: Option<Decimal>,
    pub used_amount: Option<Decimal>,
    pub limit_amount: Option<Decimal>,
    pub unit: QuotaUnit,
    pub resets_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
    pub source: DataSource,
    pub confidence: Confidence,
}
```

### Confidence

- `Exact`: Provider supplies exact value
- `Estimated`: Derived from known data
- `Partial`: Missing categories or time range
- `Unknown`: Value unavailable

## 7. Adapter lifecycle

```text
Installed
  -> Permission review
  -> Enabled
  -> Detecting
  -> Ready
  -> Collecting
  -> Healthy / Degraded / Backoff
  -> Disabled / Removed
```

Repeated crash policy:

- First failure: normal error
- Consecutive failures: backoff
- Crash threshold: adapter disabled for current session
- User sees reason and can retry
- Core remains operational

## 8. v0.1 capability matrix target

| Provider group | Local log | Hook | API | Browser | Quota | Cost |
|---|---:|---:|---:|---:|---:|---:|
| Claude | Yes | Yes | Optional | Yes | Yes | Yes |
| Codex/OpenAI | Yes | Yes | Optional | Yes | Yes | Yes |
| Cursor | Yes | No by default | Provider-dependent | Optional | Yes | Estimated/Exact |
| Gemini | Yes | Yes | Optional | Yes | Yes | Yes |
| Copilot | Yes | Provider-dependent | Optional | Optional | Yes | Estimated |
| OpenCode | Yes | Plugin/Hook with consent | No | No | Provider-dependent | Estimated |
| Grok | Yes | Yes | Optional | Yes | Yes | Yes |
| Kiro | Yes | Provider-dependent | No | Optional | Yes | Estimated |
| Ollama | Local API/log | No | Local API | No | Not applicable | Zero or user override |
| OpenRouter | No | No | Yes | Optional | Credit/limit | Exact |

Final capability must be verified against current provider behavior during implementation. Unsupported capability must be shown honestly, not fabricated.

## 9. Contract tests

Every adapter must pass:

- Detection positive fixture
- Detection negative fixture
- Incremental cursor test
- Duplicate ingestion test
- Malformed source test
- Permission denial test
- Timeout/cancellation test
- Privacy field rejection test
- Exact/Estimated/Partial labeling test
- Architecture declaration test
- Sanitized fixture scan
