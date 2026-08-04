# Changelog

All notable changes to lnwdeck will be documented in this file.

## [0.2.0] - 2026-08-04

### Highlights

- **Published provider quota.** Claude and Codex report the utilization and reset
  time of each rate-limit window to the OAuth token their own CLI already stored
  on this machine; lnwdeck reads those endpoints and shows the real percentage.
  No token count is invented around a percentage, and without a stored token the
  provider falls back to local usage windows.
- **Redesigned floating widget.** One row per window with an icon, the window
  name and description, the remaining percentage, a colour-coded bar and the next
  reset, plus a compact ring layout. Severity colours: above 50% normal, 20-50%
  warning, below 20% critical. Layout and provider selection are remembered.
- **Adapter descriptors.** Each adapter declares its source, its per-channel
  support and its authentication requirement. An unimplemented channel is
  recorded as `NOT_SUPPORTED` instead of as a successful empty collection, and an
  adapter whose source is missing reports that.
- **Seven local collectors and two credential-backed APIs.** Gemini, Cursor,
  Copilot and Kiro join OpenCode, Claude and Codex as real read-only local
  collectors. OpenRouter reports credits and rate limits, and xAI reports its
  published rate limits, both only after you store a key in Settings.
- **Honest quota data.** `limit`, `remaining` and both percentages are optional
  end to end. A window without a provider-published limit shows recorded usage;
  it never shows a bar or a percentage.
- **Costs, Models, Budgets, Alerts and Settings are real.** Costs come from the
  pricing catalog and mark unpriced models; budgets are user-configured and
  measured against recorded usage; alerts are evaluated from quota thresholds,
  collector failures and budget state; Settings persists to the database, the
  Windows Credential Manager and the Run key.
- **Redesigned interface.** Windows 11 style surfaces, dark, light and
  follow-system themes, keyboard-navigable sidebar, visible focus rings, and
  usage history and quota presented as two clearly separate channels.
- **Floating widget.** Bars and rings only where the provider published a real
  limit or percentage, reset labels that state when a reset time is unknown,
  stale and error chips, and opacity, lock, layout, size, position and provider
  selection held by the backend so the window and the dashboard cannot disagree.
- **Auto-update.** Check and install are separate commands, progress is real, a
  failed check is recorded and shown, and signature verification is performed by
  `tauri-plugin-updater` against the key in `tauri.conf.json`.

### Fixed

- Six adapters (Gemini, Cursor, Copilot, Kiro, Grok, OpenRouter) reported
  `Healthy` and returned an empty batch that the pipeline recorded as a
  successful run.
- Claude and Codex scanned their session files for quota but returned an empty
  usage batch, so no usage event was ever ingested from them.
- The webview had no Tauri capabilities, which silently disabled every
  `listen()` call, including the update banner and the widget live refresh, and
  the widget drag region.
- A quota window with an unknown limit stored `remaining_percent = 100`.
- The application shell showed a hardcoded "Fresh" badge and a timestamp taken
  from when the window opened, and discarded refresh errors.
- The Providers page used a private provider table whose ids did not match the
  adapters, so five providers could never show their detection state, and
  OpenCode was hardcoded as detected.
- `set_widget_opacity` injected CSS that the next React render overwrote.
- The update screen simulated its own states, and an unused update service
  contained a signature check that accepted any non-empty string.
- Background failures in the refresh loop, the update check and migration
  bookkeeping were dropped; they are now recorded and shown on the System page.
- The refresh loop ignored the interval the Settings page offered.

### Provider support

| Provider | Usage history | Remaining quota | Source | Needs |
|---|---|---|---|---|
| OpenCode | Local estimate | Local estimate | `opencode.db` | Local files |
| Claude | Local estimate | Published percentage per window | `api.anthropic.com/api/oauth/usage` | Claude Code sign-in |
| Codex | Local estimate | Published percentage per window | `chatgpt.com/backend-api/wham/usage` | Codex CLI sign-in |
| Gemini | Local estimate | Local estimate | `~/.gemini` records | Local files |
| Cursor | Local estimate | Local estimate | `state.vscdb` | Local files |
| Copilot | Local estimate | Local estimate | CLI and editor logs | Local files |
| Kiro | Local estimate | Local estimate | Local session records | Local files |
| Ollama | Not supported | Local / Unlimited when reachable | Local API probe | Nothing |
| OpenRouter | Not supported | Supported | `GET /api/v1/key` | API key |
| Grok | Not supported | Supported when rate limits are published | `GET /v1/api-key` | API key |

"Local estimate" means real measurements from local files with no
provider-published limit, so no remaining percentage is shown.

### Database

- Migration 004 makes quota limits nullable and rewrites stored zero limits to
  NULL, keeping the recorded usage.
- Migration 005 adds `budgets`, `alerts` and `app_events`.
- Migrations now run only when pending, inside a transaction, and record their
  version in the same transaction.
- Existing usage and quota data is preserved; the upgrade path is covered by a
  test that migrates a pre-0.2.0 database.

### Security and privacy

- Provider API keys are stored in the Windows Credential Manager and never reach
  the database, the logs or an export.
- Network access happens only for providers where a key was stored.
- Local scans are read-only and bounded by file count and byte budget; only
  numeric token counts, timestamps and model identifiers are extracted.
- Quota reports and usage batches are validated by the privacy guard before
  persistence, and rejections are recorded.

### Verification

Recorded in `docs/audits/2026-08-04-v0.2.0-audit.md` with command output: 365
Rust tests, 94 frontend tests, 3 end-to-end pipeline tests, 13 release script
tests, a clean `pnpm check`, a signed x64 build, and updater artifact
verification that checks the real signature against the shipped public key and
rejects a tampered installer.

### Known limitations

- The SQLite database is not encrypted.
- Installers carry updater signatures but are not Authenticode-signed.
- No Content-Security-Policy is configured for the webview.
- Installing an older build and letting it update itself was not executed
  locally; the verified chain stops after signature verification.
- Only the x64 bundle was built locally; ARM64 and x86 come from the release
  workflow.
- OpenCode usage events are cumulative session snapshots; per-update delta
  accounting is not implemented.
- The browser extension and native messaging host are not part of this release.

### Installation

Requirements: Windows 10 22H2 or later, WebView2 Runtime.

| Architecture | Artifact |
|---|---|
| x64 | `lnwdeck_0.2.0_x64-setup.exe` |
| ARM64 | `lnwdeck_0.2.0_arm64-setup.exe` |
| x86 | `lnwdeck_0.2.0_x86-setup.exe` |
| Any (portable) | `lnwdeck_0.2.0_portable.zip` |

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
