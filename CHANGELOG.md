# Changelog

All notable changes to lnwdeck will be documented in this file.

## [0.2.0] — 2026-08-04

### Added

- **Normalized quota reporting**: `QuotaReport` / `QuotaWindow` domain model with window scopes, kinds, reset timestamps, freshness, and confidence, replacing the primitive quota snapshot
- **Quota storage**: `quota_reports` and `quota_windows` tables (migration 003) with upsert, latest-per-provider, history, stale-marking, and pruning repository methods
- **Quota refresh channel**: usage and quota are collected and persisted independently; a quota failure never erases usage data and vice versa
- **Quota dashboard**: `get_quota_dashboard` Tauri command and `quota-updated` event, with a read model that resolves provider display names server-side
- **Floating widget on Windows**: dedicated widget window with per-provider remaining-quota bars, remaining percentage, reset countdown, stale/error/auth badges, refresh/dashboard/hide controls, lock (drag-region) mode, opacity control, and monitor-aware position persistence across restarts
- **Per-provider refresh**: `refresh_provider` command and per-card refresh buttons that no longer trigger a full pipeline refresh
- **Passive local quota/usage estimates** for OpenCode (local SQLite), Claude Code (local session JSONL), and Codex CLI (local session JSONL), clearly marked as estimates with unknown limits
- **Ollama local/unlimited status**: reachability probe reports `Local / Unlimited` when the local API is up and no fabricated quota otherwise
- **Provider contract suite**: shared invariants enforced for all ten built-in adapters (stable unique ids, sanitized detection, privacy-safe reports, no fabricated percentages)
- **Release tooling**: `scripts/check-release-version.mjs` and `scripts/generate-updater-json.mjs` with unit tests

### Changed

- Provider identifiers normalized: OpenCode now uses the canonical `opencode` id across detection, events, and quota
- Quota summary on the Providers page now comes from real quota reports instead of event counts
- Kiro is displayed as its own provider ("Kiro") instead of being mislabeled as Kimi
- Pricing never defaults unknown providers to OpenAI rates; Kiro is never priced with Kimi rates
- `refresh_all` returns a typed `{ usage, quota }` cycle instead of a run-row array
- Latest collector runs are reported per provider and collector mode (usage and quota)

### Fixed

- Production UI no longer substitutes demo data when backend commands fail; pages render explicit loading/error/empty states
- Floating widget routing: the widget now loads a dedicated `widget.html` entry instead of an incompatible hash route
- `set_widget_opacity` applies the requested value instead of ignoring it
- Widget window position is remembered across restarts and clamped back on screen when the monitor topology changes
- Kiro and Kimi pricing confusion removed
- Unknown providers are no longer charged with OpenAI rates

### Security

- Quota reports are validated by the privacy guard before persistence; paths, credentials, and account identifiers cannot enter quota data
- Local collectors are read-only, bounded (file count and total bytes), and aggregate only numeric token counts and timestamps

### Provider Support

| Provider | Usage History | Remaining Quota | Status |
|---|---|---|---|
| OpenCode | Supported (passive local scan) | Estimated (usage windows) | Stable |
| Claude | Not supported | Estimated (local usage windows) | Experimental |
| Codex | Not supported | Estimated (local usage windows) | Experimental |
| Ollama | Not supported | Local / Unlimited when reachable | Stable |
| Gemini, Cursor, Copilot, Grok, Kiro, OpenRouter | Not supported | Not supported | Not supported |

### Database

- Migration 003 adds `quota_reports` and `quota_windows` tables. Existing usage data and `quota_snapshots` rows are preserved; migrations run automatically at startup.

### Known Limitations

- OpenCode usage events are cumulative session snapshots; per-update delta accounting is not yet implemented
- Quota estimates report usage windows with unknown limits; remaining-percentage is never fabricated
- Costs, Budgets, and Alerts pages remain placeholders
- The browser extension and native messaging host are not part of this release
- Database encryption is not implemented; data is stored in local SQLite
- Installers are update-signed but not Authenticode-signed

### Upgrade Notes

- Existing local data is preserved and migrations run automatically on first launch
- Restart the application after installing the update
- Quota collectors require the provider's local CLI session data to exist (OpenCode `opencode.db`, Claude `~/.claude/projects`, Codex `~/.codex/sessions`, Ollama local API)

## [0.1.0] — 2026-08-04

### Added

- **Workspace foundation**: pnpm + Cargo monorepo with Tauri desktop app
- **Domain contracts**: privacy-safe `UsageEvent`, `UsageBatch`, `QuotaSnapshot`, `ProviderDescriptor`, `Confidence` types with JSON Schema and TypeScript contracts
- **Privacy guard**: fail-closed validation rejecting Windows/Unix paths, bearer tokens, API keys; log redaction; keyed HMAC-SHA-256 identifiers
- **SQLite storage**: transactional migrations, WAL mode, foreign keys, idempotent ingestion with backup-before-migration
- **Application layer**: ingest orchestration with privacy guard, overview read model, provider scanner
- **Provider runtime**: `ProviderAdapter` trait, adaptive scheduler with jittered backoff, deny-by-default permissions
- **Hook manager**: preview with content hash verification, atomic backup/restore, rollback cleanup
- **Browser helper**: Chromium Manifest V3 extension with native messaging host (stdio protocol, Chunked length-prefixed JSON)
- **10 built-in providers**: Claude, Codex, OpenCode, Ollama, OpenRouter, Gemini, Cursor, Copilot, Grok, Kiro
- **Pricing & analytics**: catalog with override priority, decimal-safe cost calculation, hourly/daily rollups, weighted moving average forecast
- **Desktop app**: React shell with 9 routes, tray with toggle widget, floating always-on-top widget, settings & system pages
- **Sandbox**: Wasm community adapter manifest schema, WIT interface, deny-by-default capability enforcement
- **Windows packaging**: per-user NSIS installer, portable ZIP with marker file, native messaging HKCU registration scripts
- **Background updates**: 7-state update machine, signature verification, explicit restart action, architecture-aware manifest
- **CI/CD**: quality gates (fmt/clippy/typecheck), release workflow (3-target Windows matrix), security privacy scan, dependency audit
- **E2E privacy tests**: 7 scenarios covering provider config, logs, profiles, hooks, browser messages, exports, tray data
- **Privacy scanner**: 10 forbidden patterns across all source artifacts

[0.2.0]: https://github.com/engasnm111/lnwdeck/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/engasnm111/lnwdeck/releases/tag/v0.1.0
