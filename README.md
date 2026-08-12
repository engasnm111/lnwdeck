# lnwdeck v13.0.1

Universal AI usage and quota tracker for Windows. lnwdeck reads the local
artifacts already written by AI tools, records token counts and costs, and
shows provider-reported quota. It is local-only: no account, server or cloud
sync is required.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Platform: Windows](https://img.shields.io/badge/Platform-Windows%2010%20%7C%2011-blue.svg)](https://github.com/engasnm111/lnwdeck)
[![Stack](https://img.shields.io/badge/Stack-Tauri%20%7C%20Rust%20%7C%20React%20%7C%20TypeScript-orange.svg)](https://github.com/engasnm111/lnwdeck)

## What it does

- Records local usage history: requests, input/output tokens, models and
  timestamps. Scans are read-only and bounded.
- Separates provider-reported quota from usage history. Missing provider limits
  are shown as usage-only data, never as an invented percentage.
- Calculates costs from the bundled local pricing catalog, with explicit
  estimates for models that have no exact catalog match.
- Provides budgets, alerts, diagnostics and sanitized JSON export.
- Includes a floating quota widget and a transparent desktop pet, both using
  the same live provider data as the dashboard.

## v13.0.1 update checks work where github.com is blocked

This release fixes update checks on machines where `github.com` is blocked or
degraded (the GitHub Asia node was returning empty replies, so the app could
not fetch `latest.json` and reported UPDATE_FAILED).

- **CDN fallback endpoints.** The updater tries three manifest endpoints in
  order: the GitHub release page, `raw.githubusercontent.com`, and jsDelivr.
  The release workflow commits `latest.json` to the repository root so the
  fallback endpoints always serve the current manifest.
- **API-based downloads.** `latest.json` references each installer by its
  GitHub API asset id, which redirects to the same signed binary and works
  where `github.com` is unreachable.
- **Correct download headers.** The updater sends `Accept:
  application/octet-stream` so the GitHub API redirects to the binary instead
  of returning JSON metadata.
- **Signature verification unchanged.** Installers are still verified against
  the embedded public key before installation; only the transport changed.

See [the release notes](docs/releases/v13.0.1.md), the
[end-user guide](docs/END_USER_GUIDE.md) (nine languages) and the
[provider setup guide](docs/PROVIDER_QUOTA_SETUP.md).

## v12.0.0 new LD icon and logo pack

This release replaces every application icon with a refreshed, high-contrast
`LD` monogram pack so lnwdeck is clearly visible on bright and dark Windows
wallpapers, in the taskbar, Start Menu, Store listing and the installer.

- **New monogram artwork.** A dark rounded-square tile with a high-contrast `LD`
  monogram replaces the previous icons; small app icons no longer carry a tiny
  wordmark, keeping them legible at 16px and below.
- **Every surface updated.** Taskbar sizes (16–256px and `@2x`), `icon.ico` /
  `icon.png`, `logo.ico` / `logo.png`, the Store tile set
  (Square30–Square150 and StoreLogo) and the installer icon all use the new
  pack.
- **Preview included.** `PREVIEW.png` and a README describing the pack ship
  with the icon source in `apps/desktop/src-tauri/icons`.

See [the release notes](docs/releases/v12.0.0.md), the
[end-user guide](docs/END_USER_GUIDE.md) (nine languages) and the
[provider setup guide](docs/PROVIDER_QUOTA_SETUP.md).

## v11.0.5 account-aware quota and desktop correctness

This patch adds account-aware quota and usage storage, keeps different
provider fingerprints separate, prevents duplicate desktop processes, and
restores OpenCode Go's monthly window when the dashboard omits its reset time.
The pet tooltip is wider and more compact so provider and window names remain
readable. See [the release notes](docs/releases/v11.0.5.md) and the
[provider setup guide](docs/PROVIDER_QUOTA_SETUP.md) for App/CMD/WSL and
OpenCode Go setup details.

## v11.0.4 clean-runner verification correctness

This patch keeps the OpenCode `NotConfigured` behavior covered on the same
clean-runner state used by hosted CI:

- Fixture databases without an OpenCode Go credential may truthfully report
  `NotConfigured`; the test no longer treats that state as a failure.
- The release workflow is now followed by a main-branch CI gate before a new
  release tag is created.
- The v11.0.2 quota truth and pet readability changes remain unchanged.

## v11.0.3 OpenCode clean-machine correctness

This patch keeps the v11.0.2 quota truth and readability work correct on a
machine that does not have OpenCode installed or configured:

- OpenCode (Go) keeps an explicit `NOT_CONFIGURED` diagnostic when its local
  store and per-machine workspace credentials are both absent.
- A missing OpenCode Go credential produces no unusable quota report; the
  refresh pipeline records the error and leaves the provider card in its
  localized no-connection/not-configured state.
- Regression tests cover the clean-machine detection, health and quota
  contract paths used by hosted CI.

## v11.0.2 provider quota and pet fixes

This patch release makes provider status truthful across machines and keeps
the quota labels readable in the widget and desktop pet:

- Provider quota bars now use provider-reported limits only. Missing providers
  show `No connection`, unsupported quota shows a localized usage-only state,
  and a missing provider no longer makes the combined refresh look failed.
- OpenCode (Go) requires the per-machine `OPENCODE_GO_WORKSPACE_ID` and
  `OPENCODE_GO_AUTH_COOKIE` pair, from Settings or that machine's environment;
  it never falls back to a fabricated 100%.
- Claude, Codex, Gemini, Cursor, ZCode and Kimi use corrected provider quota
  sources. Kimi follows the TokenTracker-compatible usage response and keeps
  OAuth refresh credentials in memory only.
- Pet quota rows show the complete provider and window name, such as
  `OpenCode (Go) — 5-hour`, in a wider, keyboard-readable tooltip.
- The detailed setup matrix, environment procedure and provider limitations are
  documented in [`docs/PROVIDER_QUOTA_SETUP.md`](docs/PROVIDER_QUOTA_SETUP.md).

## v11.0.1 dashboard

The Overview page is a local TokenTracker-style analytics dashboard. It combines
usage from every detected AI/provider without dropping partial results and
supports **Day**, **Week**, **Month**, **Year**, **Total** and **Custom** ranges.
The preset ranges are trailing windows ending today in the user's local
timezone: Day is today, Week is the latest 7 days, Month is the latest 30 days,
and Year is the latest 365 days. Custom ranges use the local calendar as
`[start, end)` and convert the bounds to UTC before querying. Provider filters
apply to the full result.

The page shows total/input/output tokens, duration, session count, provider cards
for **All** and each provider, a usage-trend chart, an activity heatmap and a
fixed-height **Daily breakdown** table with one row per calendar day. The table
is ordered newest-first, never shows future calendar days, and follows the
selected preset or custom date range. Selecting a provider icon filters the complete dashboard;
selecting **All** restores the combined view. Detailed session history remains
available in the separate Sessions page.

The Costs page has the same provider filter: the dropdown always lists every
provider with recorded events in the window, and selecting one narrows the
table (and the priced total) to that provider while the dropdown itself stays
complete.

The two OpenCode implementations are shown under distinct names so their data
is never confused: `OpenCode (Go)` for the billed Go implementation with
credits/quota, and `OpenCode (Free)` for legacy free-CLI records.

OpenCode (Go) quota comes from the authenticated workspace dashboard. On each
machine, open Settings and enter `OPENCODE_GO_WORKSPACE_ID` and
`OPENCODE_GO_AUTH_COOKIE` (the cookie may be pasted as its raw value or as
`auth=...`). For unattended or portable setup, the same two values can be
provided as environment variables on that machine; both are required and
environment values take precedence over Credential Manager. lnwdeck stores the
Settings pair only in Windows Credential Manager and does not display the
cookie after saving. Until configured, OpenCode local usage history can still
be collected, but quota is reported as `NOT_CONFIGURED` with no percentage
instead of showing a fabricated 100%.

See the detailed per-provider source matrix and PowerShell setup procedure in
[`docs/PROVIDER_QUOTA_SETUP.md`](docs/PROVIDER_QUOTA_SETUP.md).

### Token formatting

One shared formatter is used by Dashboard, Widget, Pet, Tray and ARIA labels:

- Compact values use uppercase `K/M/B/T`: `1.2K`, `3.5M`, `1.2B`, `10.2T`.
- Unnecessary `.0` decimals are removed (`1M`, not `1.0M`).
- Full values use ASCII comma grouping from 1,000 in every locale:
  `1,000`, `1,234,567`, `1,234,567,890`.
- Compact values are keyboard-accessible buttons. Click or press Enter/Space to
  show the full comma-grouped value; Escape returns to compact form.

### Pets and official Codex Pets import

The Pet page has explicit Add/Import controls and starts with eight bundled
defaults:

`youyou`, `old-bai`, `a-ti`, `sharkler`, `solaire`, `tennis-ball`,
`friend-pixel-pet`, `yae-miko`.

The v10 migration adds missing new defaults without restoring custom pets or
defaults that the user deliberately deleted. Pet management is kept on the Pet
page; Settings only contains widget settings.

Imports accept only an exact HTTPS `codex-pets.net` URL with the official
`https://codex-pets.net/#/pets/<id>` or
`https://codex-pets.net/api/pets/<id>/download` shape. The id is validated
locally, then a canonical download URL is constructed. The user-entered URL is
never fetched directly. HTTP, ports, credentials, subdomains, lookalike domains,
extra query/path data and malformed ids are rejected.

### Widget, tray and taskbar behavior

- Bars, Rings and Pet widget modes use a themed accessible dropdown with arrow
  keys, Enter/Space, Escape, outside-click handling, visible focus and reduced
  motion support. Native selects have a dark `color-scheme` fallback.
- Main, Widget, Pet and the localized tray popup are separate windows. Widget,
  Pet and Tray use `skip_taskbar`; Main is the only dashboard taskbar window.
  Closing Main hides it to the tray. Opening only a pet/widget therefore does
  not create a misleading main-app taskbar button.
- Left-clicking the native tray icon opens the popup beside the icon. The popup
  has localized metrics, compact/full token values and an Open dashboard action.
  The native tray menu and its `Sync now ({time})` label use the selected locale.
  Choosing **Check for updates** from the tray reports the result in the popup:
  a themed "up to date" banner with the running version when nothing newer
  exists, or a failure banner when the check cannot complete.
- Main UI, Widget, Pet, Tray popup, native tray, notifications, tooltips,
  placeholders, loading/empty/error states and ARIA labels are translated in
  English, Thai, Simplified Chinese, Japanese, Korean, German, French, Spanish
  and Russian. Language changes update open surfaces immediately.

### Background refresh and notifications

Refresh All is one shared background job for Main, Widget, System and Tray. It
coalesces repeated clicks, emits started/progress/completed/partial/failed
events, keeps the UI interactive, applies provider results incrementally and
preserves last-known data when a provider fails. Mark all as read uses an
optimistic UI with transactional rollback on failure.

## Provider support

Built-in adapters currently cover Claude, Codex, OpenCode, Gemini, Cursor,
Copilot, Kiro, ZCode, Z.AI (GLM), Kimi Code, Kilo CLI, Kilo Code, Mimo Code,
Roo Code, CodeBuddy, WorkBuddy, pi, oh-my-pi, Hermes, Ollama, OpenRouter and
Grok. Each adapter declares its supported usage/quota channels. Unsupported or
missing sources are reported honestly rather than recorded as successful empty
collections.

The Providers page and Dashboard use each adapter's full display name and
vendor (for example, **OpenAI Codex**) and keep internal ids such as
`openai_codex` out of user-facing labels.

Credential-backed providers use Windows Credential Manager or the provider's
own locally stored credential. Nothing is sent before the user explicitly
configures a provider that requires a key. OpenCode Go environment variables
are a compatibility path for a user-managed machine setup; do not put them in
`.env` files or commit them.

OpenCode (Go) is the browser-cookie exception: its Settings form requires the
workspace id and auth cookie because quota is published at the provider's
workspace dashboard rather than in the local SQLite history. The dashboard
request sends the cookie only to `https://opencode.ai`; the cookie never enters
SQLite, UI read models, logs or exports.

## Screenshots

Screenshots are captured from the built Windows application and therefore show
real local data:

| View | Image |
|---|---|
| Dashboard | ![Dashboard](assets/screenshots/overview_dashboard.png) |
| Providers | ![Providers](assets/screenshots/providers_page.png) |
| Costs | ![Costs](assets/screenshots/costs_page.png) |
| System diagnostics | ![System](assets/screenshots/system_diagnostics.png) |
| Floating widget | ![Widget](assets/screenshots/floating_widget.png) |
| Desktop pet | ![Pet](assets/screenshots/desktop_pet.png) |

Recapture them with `pwsh ./scripts/capture_app_screenshots.ps1 -ShowWidget`
after a release build.

## Languages and design

The nine supported languages are English, Thai, Simplified Chinese, Japanese,
Korean, German, French, Spanish and Russian. The selected language is stored in
local settings and is applied immediately to Main, Widget, Pet, Tray and native
tray labels.

The UI follows the Premium Dark Futuristic Glassmorphism system in
[docs/DESIGN.md](docs/DESIGN.md): near-black navy canvas, glass panels, cyan /
blue / violet accents, keyboard-visible focus and reduced-motion support. Dark
is the default theme; light mirrors the same component tokens.

## Quickstart

Requirements: Windows 10 22H2 or Windows 11, WebView2 Runtime, Node.js 24 with
pnpm, and Rust stable for `x86_64-pc-windows-msvc`.

```powershell
git clone https://github.com/engasnm111/lnwdeck.git
cd lnwdeck
pnpm install

# formatting, clippy with warnings denied, typecheck and project checks
pnpm check

# Rust, React/Vitest and release-script tests
pnpm test

# compiled pipeline E2E tests
pnpm test:e2e

# development desktop app with hot reload
pnpm tauri:dev
```

## Building and verifying v13.0.1

Signed installers and updater artifacts require the Tauri signing key:

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content keys/lnwdeck.key -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""
pnpm tauri:build

# Package one target as x64, arm64 or x86 portable ZIP
pwsh ./scripts/package-portable.ps1 -Arch x64

# Build the updater manifest from signed installers
# (LNWDECK_ASSET_IDS maps installer file names to GitHub asset ids when
#  api.github.com download URLs are wanted)
node scripts/generate-updater-json.mjs v13.0.1 <assets-dir> <assets-dir>/latest.json

# Verify versions, release fixtures, signatures/manifest contract and metadata
node scripts/check-release-version.mjs v13.0.1
pnpm release:test
```

The release workflow builds x64, ARM64 and x86 installers and portable ZIPs.
Each installer and portable ZIP has a `.sig`; the published assets also include
`latest.json`, `SHA256SUMS`, a CycloneDX SBOM and GitHub build provenance. The
updater manifest is also committed to the repository root (`latest.json`) so
the `raw.githubusercontent.com` and jsDelivr update endpoints stay reachable in
regions where `github.com` itself is blocked or degraded. The complete release
checklist and rollback procedure are in
[docs/releases/v13.0.1.md](docs/releases/v13.0.1.md).

## Privacy

- Local sources are read-only and bounded by file-count and byte limits.
- Only numeric token counts, timestamps, model identifiers, quota values and
  user-entered display metadata are stored. Prompts, responses, source code,
  file contents, file names, absolute paths and secrets are not stored.
- Provider API keys are stored in Windows Credential Manager, never in SQLite,
  logs, UI state or exports.
- OpenCode Go's two environment variables are read only when the user chooses
  that setup path; they are never copied into SQLite, logs, UI state or exports.
- Network requests are limited to declared provider endpoints and the explicit
  official Codex Pets import. No arbitrary user-entered pet URL is fetched.
- The local SQLite database is not encrypted at rest.

## Known limitations

- Installers carry updater signatures but are not Authenticode-signed, so
  Windows SmartScreen may warn on first run.
- Refresh cancellation is cooperative between synchronous provider collectors;
  the shared job has a bounded 60-second deadline, and a provider call already
  inside I/O cannot be force-killed safely. Previously stored data remains
  visible during partial refresh.
- OpenCode usage events are cumulative session snapshots; per-update delta
  accounting is not implemented.
- OpenCode Go reports utilization percentages and reset times, not an absolute
  dollar cap. If its dashboard HTML changes, the quota channel reports a
  schema error and does not guess a percentage.
- Kimi Code refreshes an expired access token in memory using the CLI's refresh
  credential; lnwdeck does not rewrite the provider-owned credential file.
- The browser extension must be loaded manually in Chromium Developer mode.

## License

MIT. See [LICENSE](LICENSE).
