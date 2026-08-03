# inwdeck v0.1 Design Specification

## Status

Approved design based on the product decisions made on 2026-08-03

## Summary

`inwdeck` is a Windows-first, open-source Universal AI Usage Tracker. It combines local logs, hooks, official APIs, browser extraction and local provider APIs into one normalized, privacy-safe local database. It exposes the same read models through a full Dashboard, System Tray popup and Always-on-top Floating Widget.

## Locked decisions

- Name: `inwdeck`
- License: MIT
- Desktop: Tauri + React + TypeScript + Rust
- Windows: Windows 10 22H2 and Windows 11
- Architectures: x64 + ARM64 Tier 1, x86 Compatibility Tier
- Distribution: Per-user installer and Portable ZIP
- UI: Dashboard + Tray + Floating Widget
- Browser: Edge + Chrome MV3
- Data collection: Hybrid
- Hook behavior: Passive first; consent per Provider
- Storage: Local-only SQLite
- Privacy: Metadata-only
- Analytics: Full analytics
- Pricing: Hybrid pricing engine
- Refresh: Adaptive
- Update: Background download, explicit Restart
- Extensibility: Built-in + Sandboxed Community Adapters
- v0.1 providers: Claude, Codex/OpenAI, Cursor, Gemini, Copilot, OpenCode, Grok, Kiro, Ollama, OpenRouter

## Architecture

The application is a modular monolith with strict boundaries. Domain and application layers do not depend on UI or Windows. Built-in adapters are trusted modules behind a common trait. Community adapters run as Wasm components with explicit capabilities. Browser collection is isolated in a Manifest V3 extension and Native Messaging host.

The only path into persistent storage is:

```text
Collector
 -> Normalized contract
 -> Privacy guard
 -> Deduplication
 -> Transactional repository
 -> Rollups
 -> Read models
```

## Components

### Desktop shell

- Tauri lifecycle
- Main window
- Tray
- Floating widget
- Native notifications
- Startup
- Update UI

### Core

- Provider scanning
- Collection orchestration
- Adaptive scheduling
- Health/backoff
- Ingestion
- Analytics
- Pricing
- Budget/alerts
- Export

### Storage

- SQLite
- Migrations and backup
- Raw metadata events
- Quota snapshots
- Rollups
- Price catalog
- Cost records
- Permissions
- Audit

### Browser Helper

- Edge/Chrome Manifest V3
- Optional host permissions
- Provider-specific extractors
- No remote code
- Native Messaging
- No Cookie or token export

### Hook manager

- Preview
- Backup
- Atomic apply
- Validate
- Rollback
- Restore

### Adapter runtime

- Built-in Rust adapters
- Wasm community adapters
- Deny-by-default permissions
- Timeout/memory/output limits
- Contract tests

## Data model

Persistent data includes tokens, cost, quota, reset times, timestamps, provider/tool/model identifiers, project aliases, keyed session hashes, confidence and source. Prompt, response, source code, file name, absolute path and credentials are not representable in the persistent Domain model.

## Error handling

Collectors return typed errors. Scheduler applies provider-specific retry and jitter. The UI continues to show last-good data with Fresh, Cached, Stale or Error state. Malformed provider data is isolated. Privacy violations fail closed.

## Testing

- Rust unit/integration tests
- Adapter contracts
- SQLite migration tests
- Privacy persistence scans
- React tests
- Browser extension tests
- Native Messaging protocol tests
- Playwright E2E
- Architecture build matrix
- Installer/portable smoke tests
- Performance benchmarks

## Release

GitHub Actions builds per architecture. Tier 1 is x64 and ARM64. x86 is compatibility tier. Updates are Tauri-signed and downloaded in the background. The app asks before restart. Portable builds never overwrite themselves automatically.

## Detailed references

This specification is expanded by:

- `docs/00_PROJECT_CHARTER.md`
- `docs/01_PRODUCT_REQUIREMENTS.md`
- `docs/02_SYSTEM_ARCHITECTURE.md`
- `docs/03_PROVIDER_ADAPTER_SDK.md`
- `docs/04_DATA_ANALYTICS_PRICING.md`
- `docs/05_SECURITY_PRIVACY.md`
- `docs/06_UI_UX_SPEC.md`
- `docs/07_WINDOWS_BUILD_RELEASE.md`
- `docs/08_TESTING_QA.md`
- `docs/09_OPEN_SOURCE_GOVERNANCE.md`
- `docs/10_ROADMAP.md`

## Self-review result

- No unresolved product decision blocks Foundation work
- Tagline is working copy and does not affect implementation
- Provider capabilities are intentionally verified during each adapter task
- Cloud sync and prompt storage are explicitly excluded
- Windows architecture tiers and update behavior are consistent across documents
