# lnwdeck

Universal AI usage and quota tracker for Windows. lnwdeck reads the artifacts your
local AI tools already write, records token counts and costs, and shows the
remaining quota your providers report. Everything stays on your machine: there is
no account, no server and no sync.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Platform: Windows](https://img.shields.io/badge/Platform-Windows%2010%20%7C%2011-blue.svg)](https://github.com/engasnm111/lnwdeck)
[![Stack](https://img.shields.io/badge/Stack-Tauri%20%7C%20Rust%20%7C%20React%20%7C%20TypeScript-orange.svg)](https://github.com/engasnm111/lnwdeck)

## What it does

- **Usage history** from local provider artifacts: requests, input and output
  tokens, models, and timestamps. Read-only, bounded scans.
- **Remaining quota** as reported by the provider, in a separate channel from
  usage history. A window whose limit the provider does not publish is shown as
  recorded usage, never as a percentage.
- **Costs** calculated from a local pricing catalog. A model without a catalog
  entry is listed as unpriced instead of being charged at another rate.
- **Budgets and alerts** you configure, evaluated against recorded usage, quota
  thresholds and collector failures.
- **Floating widget**: a small always-on-top window with its own entry point (no
  sidebar, no dashboard bundle). One row per quota window with an icon, the
  window name, the remaining percentage, a colour-coded bar and the next reset,
  or a compact ring layout. Bar colour follows severity: above 50% normal,
  20-50% warning, below 20% critical.
- **Diagnostics** for the database, migrations, per-provider collector runs and
  background failures, with a sanitized JSON export.

## Provider support

Each adapter declares what it can collect. The runtime never records a
successful collection for a channel that is not implemented, and an adapter
whose source is missing reports that instead of an empty success.

| Provider | Usage history | Remaining quota | Source | Needs |
|---|---|---|---|---|
| Claude | Local estimate | Published percentage per window | `api.anthropic.com/api/oauth/usage` with the token Claude Code stored | Claude Code sign-in |
| Codex | Local estimate | Published percentage per window | `chatgpt.com/backend-api/wham/usage` with the token Codex CLI stored | Codex CLI sign-in |
| OpenCode | Local estimate | Local estimate (usage windows) | `opencode.db` | Local files |
| Gemini | Local estimate | Local estimate (usage windows) | `~/.gemini` records | Local files |
| Cursor | Local estimate | Local estimate (usage windows) | `state.vscdb` | Local files |
| Copilot | Local estimate | Local estimate (usage windows) | CLI and editor logs | Local files |
| Kiro | Local estimate | Local estimate (usage windows) | Local session records | Local files |
| Ollama | Not supported | Local / Unlimited when reachable | Local API probe | Nothing |
| OpenRouter | Not supported | Supported (credits and rate limit) | `GET /api/v1/key` | API key |
| Grok (xAI) | Not supported | Supported when xAI publishes rate limits | `GET /v1/api-key` headers | API key |

"Published percentage per window" means the vendor reports how much of each
rate-limit window is used; lnwdeck shows that percentage and its reset time and
invents no token counts around it. Those two requests reuse the OAuth token the
vendor's own CLI already stored for you, so there is nothing to configure; when
no token is present the provider falls back to local usage windows.

"Local estimate" means the numbers are real measurements taken from local files,
but the provider publishes no plan limit, so no remaining percentage is shown -
the widget says "Unavailable" rather than guessing. API-key providers stay inert
until you store a key in Settings; nothing is sent anywhere before that.

## Screenshots

Screenshots are captured from the built application on Windows. Because the
dashboard only ever shows real local data, the captures reflect the state of the
machine they were taken on.

| View | Image |
|---|---|
| Overview | ![Overview](assets/screenshots/overview_dashboard.png) |
| Providers | ![Providers](assets/screenshots/providers_page.png) |
| Costs | ![Costs](assets/screenshots/costs_page.png) |
| System diagnostics | ![System](assets/screenshots/system_diagnostics.png) |
| Floating widget | ![Widget](assets/screenshots/floating_widget.png) |

Recapture them with `pwsh ./scripts/capture_app_screenshots.ps1 -ShowWidget`
after a release build; the script photographs the real windows and never mocks
data.

## Floating widget

- Always on top, frameless, remembered position, size, opacity, layout and
  provider selection.
- Drag by the header; Lock pins it in place. Refresh, Dashboard, layout switch,
  provider picker and Close are in the header. Escape closes it.
- Every bar and ring exposes an ARIA progressbar with the percentage and the
  reset time, and every control is reachable by keyboard.
- States it renders explicitly: loading, no data, no provider selected, stale,
  rate limited, not authenticated, unavailable and error. A window with no
  published limit shows "Unavailable" and a hatched track; a window with no reset
  time shows "Reset time unavailable".

## Quickstart

Requirements: Windows 10 22H2 or Windows 11, WebView2 Runtime, Node.js 22 with
pnpm, Rust stable for `x86_64-pc-windows-msvc`.

```powershell
git clone https://github.com/engasnm111/lnwdeck.git
cd lnwdeck
pnpm install

# fmt, clippy with warnings denied, and typecheck
pnpm check

# Rust unit and integration tests, frontend tests, release script tests
pnpm test

# end-to-end pipeline test through the compiled harness
pnpm test:e2e

# run with hot reload
pnpm tauri:dev
```

## Building a release

```powershell
# Signed installers and updater artifacts require the signing key
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content keys/lnwdeck.key -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""
pnpm tauri:build

# Portable archive
pwsh ./scripts/package-portable.ps1

# Updater manifest from the built, signed artifacts
node scripts/generate-updater-json.mjs v0.2.0 <assets-dir>
```

The bundler writes `*-setup.exe` next to a `*.sig` signature. The updater
verifies that signature against the public key in `tauri.conf.json` before it
installs anything;
`cargo test -p lnwdeck-desktop --test updater_artifacts` checks the same chain
against the built artifacts and asserts that a tampered installer is rejected.

## Privacy

- Local sources are opened read-only, with file-count and byte limits.
- Only numeric token counts, timestamps, model identifiers and quota values are
  stored. Prompts, responses, file contents, file names and absolute paths are
  not collected. The privacy guard rejects a batch or a quota report that would
  carry them, and the rejection is recorded.
- Provider API keys are stored in the Windows Credential Manager, never in the
  database, the logs or an export.
- Network requests happen only for the two provider APIs above (reusing the token
  their own CLI stored) and for providers where you stored a key. Nothing else
  leaves the machine.
- Data is stored in a local SQLite database. It is not encrypted at rest; see
  the limitations below.

## Known limitations

- The SQLite database is not encrypted.
- Installers carry updater signatures but are not Authenticode-signed, so
  Windows SmartScreen will warn on first run.
- The browser extension and the native messaging host in this repository are not
  part of the release.
- Gemini, Cursor, Copilot and Kiro collectors read whatever token records their
  tools happen to write locally; if a version stops writing them, the adapter
  reports that it found no records rather than guessing.
- OpenCode usage events are cumulative session snapshots; per-update delta
  accounting is not implemented.

## Documentation

- `docs/02_SYSTEM_ARCHITECTURE.md` - architecture and data flow
- `docs/03_PROVIDER_ADAPTER_SDK.md` - adapter descriptors and the contract suite
- `docs/05_SECURITY_PRIVACY.md` - privacy rules and the guard
- `docs/07_WINDOWS_BUILD_RELEASE.md` - build, sign and release
- `docs/audits/2026-08-04-v0.2.0-audit.md` - source audit behind this release

## License

MIT. See [LICENSE](LICENSE).
