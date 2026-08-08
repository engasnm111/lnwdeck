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
  sidebar, no dashboard bundle). Three layouts share the same real quota data:
  one row per quota window with an icon, the window name, the remaining
  percentage, a colour-coded bar and the next reset; a compact ring layout; or
  an animated robot pet whose mood is derived from the visible quotas. Bar
  colour follows severity: above 50% normal, 20-50% warning, below 20%
  critical.
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
| ZCode | Local estimate | Published percentage per window | `api.z.ai/api/monitor/usage/quota/limit` (Z.AI coding plan) or local `billing/balance` logs | ZCode sign-in |
| Z.AI (GLM) | Local estimate | Local estimate (usage windows) | GLM turns in Claude Code and OpenCode sessions | Local files |
| Kimi Code | Local estimate | Local estimate (usage windows) | `~/.kimi` / `~/.kimi-code` wire logs | Local files |
| Kilo CLI | Local estimate | Local estimate (usage windows) | `~/.local/share/kilo/kilo.db` | Local files |
| Kilo Code | Local estimate | Local estimate (usage windows) | `ui_messages.json` task history | Local files |
| Mimo Code | Local estimate | Local estimate (usage windows) | `~/.local/share/mimocode/mimocode.db` | Local files |
| Roo Code | Local estimate | Local estimate (usage windows) | `ui_messages.json` task history | Local files |
| CodeBuddy | Local estimate | Local estimate (usage windows) | `~/.codebuddy/projects` | Local files |
| WorkBuddy | Local estimate | Local estimate (usage windows) | `~/.workbuddy/projects` | Local files |
| pi | Local estimate | Local estimate (usage windows) | `~/.pi/agent/sessions` | Local files |
| oh-my-pi | Local estimate | Local estimate (usage windows) | `~/.omp/agent/sessions` | Local files |
| Hermes | Local estimate | Local estimate (usage windows) | `~/.hermes/state.db` | Local files |
| Ollama | Not supported | Local / Unlimited when reachable | Local API probe | Nothing |
| OpenRouter | Not supported | Supported (credits and rate limit) | `GET /api/v1/key` | API key |
| Grok (xAI) | Not supported | Supported when xAI publishes rate limits | `GET /v1/api-key` headers | API key |

"Published percentage per window" means the vendor reports how much of each
rate-limit window is used; lnwdeck shows that percentage and its reset time and
invents no token counts around it. Those requests reuse the OAuth token (Claude,
Codex) or API key (ZCode) the vendor's own CLI already stored for you, so there
is nothing to configure; when no token is present the provider falls back to
local usage windows. ZCode's quota comes from the Z.AI GLM Coding Plan monitor
API (5-hour, weekly and tool-call windows); when no API key is stored it reads
the `billing/balance` records ZCode already wrote into its own logs.

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
| Desktop pet | ![Pet](assets/screenshots/desktop_pet.png) |

Recapture them with `pwsh ./scripts/capture_app_screenshots.ps1 -ShowWidget`
after a release build; the script photographs the real windows and never mocks
data.

## Floating widget

- Always on top, frameless, remembered position, opacity, layout and provider
  selection. The window size is **fixed** to one of three presets chosen in
  Settings โ€” Small (300x300), Medium (400x420) or Large (500x500) โ€” and
  content scrolls inside the window when it outgrows the space.
- Drag by the header, or by the pet stage in pet layout; Lock pins it in place.
  Refresh, Dashboard, layout picker, provider picker and Close are in the
  header. Escape closes it.
- Three layouts chosen from the header or from Settings: bars, rings, or the
  animated pet. All three render the same quota dashboard data; the pet layout
  shows the same rows below a small robot pet whose mood reacts to the visible
  quotas (happy, worried, critical, stale, error, sleeping) and celebrates a
  successful manual refresh. The pet is decorative; the quota rows carry the
  accessible information. Rings wrap to fit narrow sizes.
- Community pets from [codex-pets.net](https://codex-pets.net) can replace the
  built-in robot: import one in Settings from a pet URL or a `.codex-pet.zip`
  file. The package is downloaded only on that explicit action, over HTTPS,
  only from codex-pets.net, validated (manifest, WebP spritesheet, size
  limits, no symlinks) and then stored locally โ€” the widget animates it from
  the local store and never loads a remote asset.
- Every bar and ring exposes an ARIA progressbar with the percentage and the
  reset time, and every control is reachable by keyboard.
- Only providers whose quota collection produced data are shown. A provider
  that failed to fetch (not configured, not authenticated, rate limited, or a
  collector error) is hidden until it recovers; open the dashboard for the
  reason. Stale readings stay visible and are labelled. The widget states are
  rendered explicitly: loading, no data, no provider selected, stale, and
  fetch error. A window with no published limit shows "Unavailable" and a
  hatched track; a window with no reset time shows "Reset time unavailable".
  Pet mode never estimates a missing percentage: providers without published
  limits stay unavailable, exactly as in bars and rings.

## Desktop pet

A floating companion that walks across your screen, powered by the same real
quota data the dashboard reads.

- **Transparent pet window** that moves with the pet: the window is small and
  fixed-size, so clicks pass through everywhere except the pet itself. Size
  presets are chosen in the Pet page โ€” Small (200x300), Medium (280x400) or
  Large (360x520) โ€” and the sprite scales with the window.
- **Walks on its own**: idle, walk left/right with an edge bounce, and
  auto-sleep after inactivity (toggleable). Speed and opacity are adjustable.
- **Hover shows every quota window** the providers published: real remaining
  percentages render as colour-coded bars, usage-only windows (no published
  limit) render as hatched "used" rows, and the list scrolls when it is long.
- **Right-click opens a menu** with Pet settings and Close pet. Left-press +
  drag picks the pet up and moves it anywhere on screen (clamped to the
  monitor).
- **Six bundled default pets** from [codex-pets.net](https://codex-pets.net)
  ship inside the binary (youyou, old-bai, a-ti, sharkler, solaire,
  tennis-ball), installed on first run with no network needed. Any community
  pet can be imported by URL or id from the Pet page; v1 and v2 spritesheets
  are both rendered as animated atlases.
- **Pet page in the sidebar** manages everything: show/hide, character
  selection with live spritesheet previews, speed, size, opacity,
  auto-sleep, and codex-pets.net imports.
- **Pose control and staying put**: each ambient pose (wave, jump, look
  left/right, waiting, review) can be toggled independently from the Pet
  page or Settings, and the pet can be set to stay in place instead of
  walking.
- **Talks back**: a tap shows a random quip built from live quota numbers
  (tokens today, lowest remaining quota, plan), localized to the UI
  language.
- **Click-through window**: only the sprite and its tooltip intercept the
  mouse; the rest of the window passes clicks to the desktop underneath.

## Browser helper

An optional Chromium extension (`apps/browser-extension`) detects quota usage
on ChatGPT, Claude and Gemini pages and forwards it to a native messaging
host that ships with the installer (`lnwdeck-browser-host.exe`). The host
records sanitized detections into the app's event log. Setup:

```powershell
# 1. Build the extension, then load `apps/browser-extension` via
#    chrome://extensions (Developer mode โ’ Load unpacked).
# 2. Register the host with the extension's real ID:
pwsh ./scripts/register-native-host.ps1 -ChromeExtensionId <id> -EdgeExtensionId <id>
```

## Languages

The UI ships in nine languages โ€” English, Thai, Chinese, Japanese, Korean,
German, French, Spanish and Russian โ€” switchable instantly in Settings.
Timestamps follow the selected language's calendar and use a 24-hour clock
with seconds (e.g. `07/08/2569 15:45:32` in Thai).

## Design system

The UI follows the "Premium Dark Futuristic Glassmorphism" design language
documented in [docs/DESIGN.md](docs/DESIGN.md): a near-black navy canvas
(`#05070F`) lit by ambient cyan, blue and violet glows, frosted glass panels
with thin glowing borders, a cyan โ’ violet accent family, and an amber โ’ red
warm gradient reserved for primary CTAs. Dark is the default theme; light
mirrors it faithfully. All motion animates only transform/opacity and is
disabled under `prefers-reduced-motion`.

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

# Portable archive (add -Arch x64|arm64|x86 when packaging a specific target)
pwsh ./scripts/package-portable.ps1

# Updater manifest from the built, signed artifacts
node scripts/generate-updater-json.mjs v0.7.1 <assets-dir>
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
- Network requests happen only for the provider APIs above (reusing the token
  their own CLI stored), for ZCode's coding-plan quota monitor
  (`api.z.ai` / `bigmodel.cn`), and for providers where you stored a key.
  Nothing else leaves the machine.
- Data is stored in a local SQLite database. It is not encrypted at rest; see
  the limitations below.

## Known limitations

- The SQLite database is not encrypted.
- Installers carry updater signatures but are not Authenticode-signed, so
  Windows SmartScreen will warn on first run.
- The browser extension must be loaded manually (Developer mode); the native
  messaging host binary itself ships with the installer.
- Gemini, Cursor, Copilot, Kiro and the v0.8 local collectors read whatever
  token records their tools happen to write locally; if a version stops writing
  them, the adapter reports that it found no records rather than guessing.
- OpenCode usage events are cumulative session snapshots; per-update delta
  accounting is not implemented.

## Credits

lnwdeck builds on the work of great open-source projects and communities:

- [TokenTracker](https://github.com/xiufengsun/TokenTracker) - reference for
  the community pet package format (`pet.json` + WebP spritesheet), the
  codex-pets.net import validation rules, and the floating desktop pet
  window approach.
- [tokscale](https://github.com/junhoyoe/tokscale) - reference for reading
  Codex CLI usage records.
- [codex-pets.net](https://codex-pets.net) - the community pet catalog that
  ships the eight bundled default pets and powers the pet import feature.
- The [Tauri](https://tauri.app) project and its plugin ecosystem.

## License

MIT. See [LICENSE](LICENSE).
