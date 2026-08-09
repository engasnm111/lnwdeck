# Changelog

All notable changes to lnwdeck will be documented in this file.

## [11.0.3] - 2026-08-09

### Correctness

- OpenCode (Go) now keeps `NOT_CONFIGURED` when neither its local store nor
  its per-machine workspace credentials exist, so clean machines are shown as
  not connected instead of silently clearing the diagnostic state.
- Missing OpenCode Go credentials return a quota collection error without an
  unusable quota report, satisfying the provider contract and preventing fake
  quota data from entering the refresh pipeline.

### Verification

- Added regression coverage for clean-machine detection, health status and
  missing-credential quota collection.
- Hosted provider contract coverage now passes without OpenCode installed or
  configured on the runner.

## [11.0.2] - 2026-08-09

### Highlights

- Provider quota is now truth-preserving: absent providers show a localized
  no-connection state, unsupported providers remain usage-only, and refresh does
  not report a global failure just because a provider is not installed.
- OpenCode (Go) now requires a workspace id plus auth cookie on each machine;
  quota is read from the authenticated workspace dashboard and never invented
  from local token totals.
- Claude, Codex, Gemini, Cursor, ZCode and Kimi quota adapters use corrected
  provider sources, with Kimi matching the TokenTracker usage response and
  keeping refreshed OAuth tokens in memory.
- Pet quota tooltips show the full provider and quota window name instead of
  truncating the label, including the 5-hour, 7-day and 30-day windows.

### Documentation

- Added the detailed provider quota matrix and per-machine OpenCode Go setup
  guide in `docs/PROVIDER_QUOTA_SETUP.md`.
- Added release notes and rollback guidance in `docs/releases/v11.0.2.md`.

## [11.0.1] - 2026-08-09

### Highlights

- Codex quota now uses the live provider API first, with a local JSONL snapshot
  fallback only for unavailable-provider failures; authentication and rate-limit
  errors remain visible instead of being hidden by stale local data.
- Pet speech bubbles keep their complete clickable surface and wrap long text
  without leaving a clipped or broken frame.
- The desktop pet, quota widget and tray popup stay off the Windows taskbar;
  only the visible dashboard owns a taskbar button.
- Release builds now use tuned Cargo profiles, `rust-lld`, target-specific
  caches, CI `sccache` and a single NSIS bundle path to reduce release build
  time and duplicate work.

### Verification

- Focused Codex provider tests: 25/25 passed.
- Desktop Rust tests: 39/39 passed.
- Release/version/workflow tests: 31/31 passed.
- Windows x64 release build completed with the tuned linker/profile.

## [11.0.0] - 2026-08-09

### Highlights

- Costs page provider filter: the dropdown always lists every provider with
  recorded events in the window, and selecting one narrows the table and the
  priced total to that provider in every locale.
- OpenCode implementations are shown under distinct names: `OpenCode (Go)` for
  the billed Go implementation with credits/quota, and `OpenCode (Free)` for
  legacy free-CLI records, so the two data sets are never confused.
- Tray **Check for updates** reports its result in the themed tray popup: an
  "up to date" banner with the running version when nothing newer exists, or a
  failure banner when the check cannot complete. Translated in all nine locales.
- Settings **Show the floating quota widget** now shows/hides the native widget
  window immediately (same commands as the tray), instead of only persisting a
  setting that applied at the next restart.

## [10.0.0] - 2026-08-08

### Highlights

- TokenTracker-style Dashboard with calendar ranges, provider filtering, total
  token/input/output/duration/session summary, usage trend, activity heatmap and
  session-level provider breakdown.
- Shared compact/full token formatter with uppercase `K/M/B/T` and ASCII comma
  grouping from 1,000; compact values toggle by click and keyboard.
- Eight bundled pets, migration-safe defaults, Pet-page Add/Import controls and
  strict official Codex Pets URL validation.
- Complete nine-locale Main/Widget/Pet/Tray/notification coverage and a themed,
  keyboard-accessible Widget mode dropdown.
- Shared background Refresh All job with progress/partial results and a
  transactional Mark all as read notification action.
- Main-only taskbar behavior; Widget, Pet and Tray are hidden from the taskbar.

### Release assets

- v10.0.0 targets x64, ARM64 and x86 Windows builds.
- The release workflow verifies detached signatures and `SHA256SUMS`, publishes
  `latest.json`, a CycloneDX SBOM and GitHub build provenance.
- Full installation, migration, rollback and verification details:
  [docs/releases/v10.0.0.md](docs/releases/v10.0.0.md).

## [0.9.0] - 2026-08-08

### Highlights

- **Sessions page — token usage per project folder.** A new page groups usage
  by session and folder (privacy-safe keyed hashes only; raw paths and session
  ids are never stored). Sessions and folders have generated names
  (`Project 01`, `Session 01`) that the user can rename inline; records
  without attribution land in an *Unassigned* bucket. OpenCode attributes
  every event already; other adapters follow.
- **Costs page finally shows prices.** The bundled price catalog grew from 37
  to **1,389 models across 11 providers** (generated from the MIT-licensed
  LiteLLM price snapshot), matching now tolerates provider aliases,
  vendor-prefixed ids and dated model variants, and anything still unknown is
  charged a labeled generic estimate instead of a blank cell — every model
  has a cost, never an unrelated provider's rate.
- **Dark-mode dropdown fix.** The native select popup no longer renders white
  in dark theme; option rows are painted with opaque theme colors.
- **Two new bundled default pets** (Friend Pixel Pet, Yae Miko) from
  codex-pets.net, installed offline on first run next to the original six.

### Added

- Sessions read model (`get_sessions`) and rename commands
  (`rename_session`, `rename_project`); migration v006 adds `session_hash`
  and `project_hash` attribution columns plus session/project name metadata.
- OpenCode adapter attributes every event with session and project keyed
  hashes; the privacy scan confirms raw ids never leave the adapter.
- Pricing catalog generator script (`scripts/update-pricing-catalog.mjs`).
- Pricing estimate status: unknown models are charged the labeled generic
  estimate, and the Costs page marks them with an *estimated* badge.

### Fixed

- Costs/Budgets/Overview pages showed no cost for models the old 37-model
  catalog did not cover exactly.
- Dark theme: white dropdown popup made option text unreadable.
- Sessions list in dark mode now matches the theme.
## [0.8.0] - 2026-08-08

### Highlights

- **Z.AI quota, for real.** The new ZCode adapter reads the GLM Coding Plan
  quota ZCode stores locally and reports the published 5-hour, weekly and
  tool-call windows with real percentages and reset times (monitor API when a
  plaintext key is in `~/.zcode/v2/config.json`, otherwise the
  `billing/balance` records ZCode already wrote to its own logs), falling back
  to usage-only local windows. The new Z.AI adapter estimates GLM usage from
  Claude Code and OpenCode sessions wherever the Coding Plan is used, and the
  pricing catalog now covers the GLM model family.
- **12 new provider adapters** — parity with the TokenTracker tool list for
  every passive local source: ZCode, Z.AI (GLM), Kimi Code, Kilo CLI, Kilo
  Code, Mimo Code, Roo Code, CodeBuddy, WorkBuddy, pi, oh-my-pi and Hermes.
  All are read-only scans of files the tools already write; nothing is
  installed into any tool and no credentials are read.

### Added

- **ZCode** (`zcode_ai`): usage from `~/.zcode/cli/db/db.sqlite` (bundled
  Claude/Codex/Gemini sub-agent turns excluded), quota from the Z.AI / BigModel
  coding-plan monitor API or local balance logs.
- **Z.AI** (`zai_glm`): GLM usage across Claude Code JSONL and the OpenCode
  message table (`opencode-go` turns excluded), usage-only quota windows.
- **Kimi Code** (`kimi_code`): `wire.jsonl` reader for both the legacy
  `~/.kimi` StatusUpdate layout and the official `~/.kimi-code` `step.end`
  layout with `modelAlias`.
- **Kilo CLI** (`kilo_cli`): OpenCode-fork store at
  `~/.local/share/kilo/kilo.db`.
- **Mimo Code** (`mimo_code`): OpenCode-fork store at
  `~/.local/share/mimocode/mimocode.db`; only native `mimo` / `xiaomi` turns
  are counted, mirrored Claude history is excluded.
- **Kilo Code** (`kilo_code`) and **Roo Code** (`roo_code`): Cline-derived
  `ui_messages.json` task readers; Roo uses the last `<model>` tag from
  `api_conversation_history.json` and falls back to `protocol:<apiProtocol>`.
- **CodeBuddy** (`codebuddy`), **WorkBuddy** (`workbuddy`), **pi** (`pi_agent`)
  and **oh-my-pi** (`omp`): Claude-fork JSONL session scans.
- **Hermes** (`hermes`): read-only `state.db` sessions reader
  (`~/.hermes/state.db` or `%LOCALAPPDATA%\hermes\state.db`).
- **Pricing catalog**: `zai` provider entry with the published GLM price list,
  `kimi` entries for the K2 family, and provider normalization for
  `zcode_ai` / `zai_glm` / `glm-*` models.
- **Shared parser infrastructure**: OpenCode-fork `message` table reader and
  Cline `ui_messages.json` parser in `lnwdeck-provider-runtime`, plus raw
  `authorization` header support in `lnwdeck-provider-http`.

### Verification

- 60 new unit tests across the adapter crates and shared parsers; contract
  suite extended from 10 to 22 adapters and passing; `cargo fmt --check`,
  `cargo clippy --workspace --all-targets --all-features -- -D warnings` and
  the full workspace test suite pass locally.

## [0.7.1] - 2026-08-08

### Fixed

- **Budget form alignment.** Budget fields now share a stable five-column grid
  with aligned labels and hints, while the enabled toggle and save action live
  in a dedicated action row that collapses cleanly on smaller screens.
- **Refresh All responsiveness.** Manual refresh now returns after a bounded
  60-second timeout, reports the failure, and keeps the in-flight worker guard
  until the blocking collector actually exits so a timed-out cycle cannot
  overlap a new one.
- **Alert acknowledgement state.** The navigation badge now counts
  unacknowledged alerts and updates immediately when the Alerts page
  acknowledges one.

### CI/CD

- Build-heavy CI jobs use the Windows runner proven by the release workflow.
- Per-architecture compile checks no longer build every test target; the
  dedicated test job remains responsible for test-target coverage.

### Verification

- Workspace check, frontend tests, desktop Rust tests, release script tests and
  pipeline E2E tests pass locally.

## [0.7.0] - 2026-08-07

### Highlights

- **Premium Dark Futuristic Glassmorphism.** A full visual redesign of every
  window: near-black navy canvas with ambient cyan/blue/violet glows, frosted
  glass panels with thin glowing borders, a cyan → violet accent family, and
  an amber → red warm gradient reserved for primary CTAs. The sidebar, topbar,
  cards, tables, controls, floating widget and tray popup all follow the new
  language documented in `docs/DESIGN.md`.
- **Atmospheric background.** Drifting gradient orbs, a faded grid and an
  inline SVG noise grain sit behind the dashboard; motion is transform/opacity
  only and fully disabled under `prefers-reduced-motion`.
- **Premium micro-interactions.** Card hover lifts with a cyan glow, primary
  buttons sweep a shine across the warm gradient, progress bars and toggles
  carry the cyan → blue gradient, active navigation glows cyan → violet.
- **Widget restyled to match.** The floating widget is a frosted navy glass
  card with a glowing border and the dashboard's cyan → violet top-edge
  accent.
- **CI/CD reliability.** The Windows runner no longer kills cargo silently
  mid-build: Defender exclusions, an enlarged pagefile, retrying cargo steps
  with memory/disk diagnostics, and LLVM for the ARM64 compile job. Release
  publishes a portable ZIP per architecture instead of one overwriting all
  three.

## [0.4.0] - 2026-08-05

### Highlights

- **Animated pet layout for the floating widget.** A third layout, chosen from
  the header next to bars and rings, shows a small robot pet above the same
  quota rows as bars mode. The pet's mood is derived purely from the visible
  quota data: happy above 50% remaining, worried at 20-50%, critical below 20%,
  puzzled when a reading is stale, sad on auth, rate-limit or collection
  errors, and sleeping when no visible provider published a real percentage.
  A successful manual refresh triggers a brief celebration that returns to the
  derived mood on its own. The pet never estimates a missing percentage and
  the decorative artwork is hidden from assistive technology; the quota rows
  carry all accessible information.
- **Redesigned UI — Graphite & Indigo.** A full visual refresh of the design
  system behind every window: a deeper graphite-blue dark palette with a
  refined indigo accent, a clearer type scale, softer radii and layered
  elevation, consistent focus rings and hover states on every control, a
  polished sidebar with a brand mark and smoother active indicators, and a
  topbar that matches the panel language of the widget. All tokens keep their
  names, so pages and shared components inherit the new language without
  structural changes, and every motion still respects
  `prefers-reduced-motion`.
- **Three-way layout picker.** The widget header now offers Bars, Rings and
  Pet explicitly instead of a two-state toggle, and an invalid stored layout
  still falls back to bars.
- **Widget shows only fetched quota.** A provider whose quota collection
  failed (not configured, not authenticated, rate limited, or a collector
  error) is hidden from the widget until it recovers; the dashboard explains
  the reason. Stale readings remain visible and labelled.
- **Community pets from codex-pets.net.** The widget pet can be replaced by a
  community pet imported from a codex-pets.net URL or a local
  `.codex-pet.zip`. Imports only happen on an explicit action, over HTTPS,
  only against codex-pets.net; every package is validated (pet.json manifest,
  WebP spritesheet, size limits, no symlinks) before it is stored locally and
  served to the widget through the local `petlocal://` protocol — no remote
  asset ever reaches the webview. The pet animates with the same CSS
  frame-cycling as the built-in robot and honors reduced motion.

### Notes

- The pet stays inside the widget and does not roam across the desktop; it has
  no click interactions or sounds in this release.

## [0.3.0] - 2026-08-05

### Highlights

- **OpenCode Go quota with real dollar caps.** The OpenCode adapter now reads
  the Go turns recorded in the local `message` table and compares their billed
  USD cost against the published Go caps ($12 per 5 hours, $30 per week, $60
  per month), reporting windows with real limits, remaining and percentages.
  When no Go turns are recorded it falls back to usage-only token windows.
- **Gemini usage from the real session transcripts.** Gemini CLI stores one
  transcript per chat under `~/.gemini/tmp/<project>/chats/`; every model
  message carries cumulative token counters, so per-message usage is the delta
  between two consecutive counters. The adapter streams those transcripts
  read-only — including the tail of very large files — and now reports real
  usage instead of "no token records". Prompts, responses and paths never
  leave the source.
- **Cursor usage and quota from its account API.** Cursor keeps no per-request
  token data in the local editor state, so the adapter reuses the session JWT
  Cursor's own tooling stores in `state.vscdb` and reads the account API: the
  per-request usage CSV and the utilization summary (Plan / Auto / API lanes,
  billing-cycle reset). The credential only travels over HTTPS to cursor.com
  and never enters lnwdeck storage, logs or the UI.

### Fixed

- Gemini reported "no token records" on machines with real usage because the
  generic JSON scan targeted a shape the CLI never writes.
- Cursor reported "no token records" because per-request usage is not stored
  in `state.vscdb` at all.
- OpenCode quota could never show a remaining percentage because the local
  estimate had no limit.
- A storage diagnostics test used a fixed retry date and broke as soon as the
  real clock passed it.

## [0.2.1] - 2026-08-04

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
  end to end. A window is shown as a bar only when the provider published a limit
  or a percentage; otherwise it shows the recorded usage and says the limit is
  unavailable.
- **Costs, Models, Budgets, Alerts and Settings are real.** Costs come from the
  pricing catalog and mark unpriced models; budgets are user-configured and
  measured against recorded usage; alerts are evaluated from quota thresholds,
  collector failures and budget state; Settings persists to the database, the
  Windows Credential Manager and the Run key.
- **Redesigned interface.** Windows 11 style surfaces, dark, light and
  follow-system themes, keyboard-navigable sidebar, visible focus rings, and
  usage history and quota presented as two clearly separate channels.
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
- Network access happens only for the two published-quota endpoints above, using
  the token the vendor's own CLI stored, and for providers where a key was stored.
- Local scans are read-only and bounded by file count and byte budget; only
  numeric token counts, timestamps and model identifiers are extracted.
- Quota reports and usage batches are validated by the privacy guard before
  persistence, and rejections are recorded.

### Verification

Recorded in `docs/audits/2026-08-04-audit.md` with command output: 392
Rust tests, 112 frontend tests, 3 end-to-end pipeline tests, 13 release script
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
| x64 | `lnwdeck_0.2.1_x64-setup.exe` |
| ARM64 | `lnwdeck_0.2.1_arm64-setup.exe` |
| x86 | `lnwdeck_0.2.1_x86-setup.exe` |
| Any (portable) | `lnwdeck_0.2.1_portable.zip` |

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

[0.7.1]: https://github.com/engasnm111/lnwdeck/compare/v0.7.0...v0.7.1
[0.2.1]: https://github.com/engasnm111/lnwdeck/compare/v0.1.0...v0.2.1
[0.1.0]: https://github.com/engasnm111/lnwdeck/releases/tag/v0.1.0
