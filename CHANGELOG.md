# Changelog

All notable changes to inwdeck will be documented in this file.

## [0.1.0] — Unreleased

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
