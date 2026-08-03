# inwdeck

> **Universal AI Usage Tracker for Windows**  
> Aggregate tokens, costs, quotas, and reset windows across all your AI tools and providers in a single, privacy-first local dashboard.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Platform: Windows](https://img.shields.io/badge/Platform-Windows%2010%20%7C%2011-blue.svg)](https://github.com/engasnm111/lnwdeck)
[![Architecture: Tauri + Rust + React](https://img.shields.io/badge/Stack-Tauri%20%7C%20Rust%20%7C%20React%20%7C%20TypeScript-orange.svg)](https://github.com/engasnm111/lnwdeck)

---

## 🌟 Overview

**inwdeck** is an open-source, local-first application for Windows designed to bring all your AI provider quotas, token usage, costs, and reset schedules into a single unified view. Whether you use CLI tools, local models, web interfaces, or IDE extensions, **inwdeck** detects and collects metadata locally without sacrificing privacy.

### Key Highlights

- **🔒 Local-First & Metadata-Only**: Operates 100% locally with zero cloud servers or user accounts required in v0.1. **inwdeck** collects only usage metrics—**never** your prompts, AI responses, source code, file contents, or personal paths.
- **📊 Unified Multi-View UI**:
  - **Main Dashboard**: Detailed analytics, historical charts, cost breakdown, and budget forecasts.
  - **System Tray Popup**: Quick glance at remaining quotas and upcoming reset windows.
  - **Floating Widget**: Customizable overlay for live token and cost tracking while coding.
- **🔌 Hybrid Data Collection**: Aggregates metrics via local session logs, file watchers, passive hooks, official APIs, Chromium Browser Helper (Edge/Chrome), and sandboxed community adapters.
- **💰 Hybrid Pricing Engine**: Combines an offline pricing database with online sync and custom user overrides for accurate cost estimation.
- **🛡️ Sandboxed Adapters**: Community adapters run in isolated sandboxes with deny-by-default permissions for network, file, and credential access.

---

---

## 🛠️ Getting Started

### Prerequisites

- **Windows 10 22H2+** or **Windows 11**
- **Node.js 22+** (with pnpm)
- **Rust 1.77+** (stable-x86_64-pc-windows-msvc)

Install tools:

```powershell
# Install Rust (if not installed)
Invoke-WebRequest -Uri "https://win.rustup.rs/x86_64" -OutFile "$env:TEMP\rustup-init.exe"
& "$env:TEMP\rustup-init.exe" -y

# Install pnpm (if not installed)
npm install -g pnpm

# Clone and enter project
git clone https://github.com/engasnm111/inwdeck.git
cd inwdeck
```

### Development

```powershell
# Install all dependencies (Rust + Node.js)
pnpm install

# Run quality checks (fmt + clippy + typecheck)
pnpm check

# Run all tests (Rust + TypeScript + E2E)
pnpm test

# Run privacy scanner
node scripts/scan-artifacts-for-sensitive-data.mjs

# Launch in dev mode (hot-reload)
pnpm tauri:dev
```

### Build

```powershell
# Build release artifacts
pnpm tauri:build

# Create portable ZIP
pwsh ./scripts/package-portable.ps1
```

### Project Structure

```
inwdeck/
├── apps/
│   ├── desktop/               Tauri + React application
│   │   ├── src/               React UI (pages, components, routes)
│   │   └── src-tauri/         Tauri backend (Rust commands, state)
│   ├── browser-extension/     Chrome/Edge Manifest V3 extension
│   └── native-messaging-host/ Native messaging stdio bridge (Rust)
├── crates/
│   ├── domain/                Domain types (UsageEvent, QuotaSnapshot, ...)
│   ├── security/              Privacy guard, redaction, keyed hashing
│   ├── storage/               SQLite, migrations, repositories
│   ├── application/           Use cases (ingest, overview, update)
│   ├── provider-runtime/      Adapter trait, scheduler, Wasm sandbox
│   ├── hook-manager/          Safe config mutation (preview/backup/rollback)
│   ├── pricing/               Price catalog and cost calculation
│   ├── analytics/             Rollups, budgets, forecast
│   ├── windows-integration/   Windows credential model
│   └── providers/             10 built-in adapters (claude, codex, ...)
├── packages/
│   ├── contracts/             Generated TypeScript contracts
│   └── ui/                    Shared UI components (DataState, ...)
├── schemas/                   JSON Schema + WIT interfaces
├── scripts/                   Quality check, packaging, privacy scan
├── e2e/                       End-to-end privacy tests
├── assets/                    Pricing catalog
├── installer/                 Package configuration
└── .github/workflows/         CI (check + test + release + security)
```

## 🚀 Supported AI Providers (v0.1)

1. **Claude Code / Claude Web**
2. **Codex CLI / ChatGPT**
3. **Cursor**
4. **Gemini CLI / Gemini Web**
5. **GitHub Copilot**
6. **OpenCode**
7. **Grok Build / Grok Web**
8. **Kiro**
9. **Ollama** (Local models)
10. **OpenRouter**

*Supports custom/community adapters via the inwdeck Provider SDK.*

---

## 🛠️ Architecture & Tech Stack

- **Desktop Framework**: [Tauri](https://tauri.app/) (Rust Core + Native OS Integration)
- **Frontend**: React, TypeScript, Vanilla CSS (Offline-capable)
- **Storage**: Embedded SQLite (Metadata only, ACID compliant, local migrations)
- **Browser Helper**: Chromium Manifest V3 Extension (Microsoft Edge & Google Chrome)
- **Supported Architectures**:
  - **x64 & ARM64**: Tier 1 (Fully supported & automated build checks)
  - **x86**: Compatibility Tier

---

## 🔒 Security & Privacy Commitments

- **Zero Cloud Sync**: Your data never leaves your computer.
- **Strict Data Redaction**: Log files and database entries filter out prompts, responses, credentials, and full file paths before writing to disk.
- **Permission Consent**: Passive monitoring by default. Installing active hooks or granting adapter permissions requires explicit user preview, backup, and approval.
- **Secure Secret Storage**: Credentials (such as API keys) are stored strictly in Windows Credential Manager or Tauri Stronghold—never in plain text files, source code, or SQLite.

---

## 📚 Documentation

Detailed documentation is available in the [`docs/`](docs) directory:

| Document | Description |
|---|---|
| [`00_PROJECT_CHARTER.md`](docs/00_PROJECT_CHARTER.md) | Vision, principles, scope, and non-goals |
| [`01_PRODUCT_REQUIREMENTS.md`](docs/01_PRODUCT_REQUIREMENTS.md) | Core requirements and acceptance criteria |
| [`02_SYSTEM_ARCHITECTURE.md`](docs/02_SYSTEM_ARCHITECTURE.md) | Process, module, and data flow architecture |
| [`03_PROVIDER_ADAPTER_SDK.md`](docs/03_PROVIDER_ADAPTER_SDK.md) | Adapter contract, permissions, and sandbox specs |
| [`04_DATA_ANALYTICS_PRICING.md`](docs/04_DATA_ANALYTICS_PRICING.md) | SQLite schema, pricing engine, and forecasting |
| [`05_SECURITY_PRIVACY.md`](docs/05_SECURITY_PRIVACY.md) | Threat model, redaction rules, and security guidelines |
| [`06_UI_UX_SPEC.md`](docs/06_UI_UX_SPEC.md) | Specs for Dashboard, Tray Popup, and Floating Widget |
| [`07_WINDOWS_BUILD_RELEASE.md`](docs/07_WINDOWS_BUILD_RELEASE.md) | Installer, Portable builds, and Auto-update design |
| [`08_TESTING_QA.md`](docs/08_TESTING_QA.md) | Test strategy, CI setup, and privacy verification |
| [`09_OPEN_SOURCE_GOVERNANCE.md`](docs/09_OPEN_SOURCE_GOVERNANCE.md) | Governance, contribution rules, and license terms |
| [`10_ROADMAP.md`](docs/10_ROADMAP.md) | Milestones and future roadmap |

---

## 🤝 Contributing

We welcome contributions from the community! Please read [`docs/09_OPEN_SOURCE_GOVERNANCE.md`](docs/09_OPEN_SOURCE_GOVERNANCE.md) and [`AGENTS.md`](AGENTS.md) for contribution guidelines, code standards, and developer rules.

### Community Adapters

Interested in adding support for a new AI provider? Check out [`docs/03_PROVIDER_ADAPTER_SDK.md`](docs/03_PROVIDER_ADAPTER_SDK.md) to build sandboxed provider adapters.

---

## 📄 License

**inwdeck** is licensed under the [MIT License](LICENSE).
