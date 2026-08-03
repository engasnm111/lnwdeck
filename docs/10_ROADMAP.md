# inwdeck v0.1 Roadmap

## Milestone 0 — Foundation

Deliverable: Repository builds and opens a named `inwdeck` Tauri app

- Monorepo
- Rust workspace
- React application
- Shared commands
- CI
- Formatting/linting/testing
- Basic Tray lifecycle
- Settings and data directory abstraction
- Architecture documentation

Exit gate:

- x64 build works locally
- ARM64/x86 compile jobs defined
- Tests and lint run from root
- Closing window keeps Tray app alive

## Milestone 1 — Domain, Storage and Privacy

Deliverable: Safe metadata ingestion and queryable SQLite

- Domain types
- Privacy guard
- SQLite migrations
- Repositories
- Deduplication
- Sync cursors
- Local hashing key
- Backup/recovery
- Overview query

Exit gate:

- Forbidden fields rejected
- Duplicate batch is idempotent
- Migration rollback test passes
- 1 million event query benchmark recorded

## Milestone 2 — Provider Runtime

Deliverable: Adapter lifecycle and adaptive scheduler

- Adapter trait
- Detection
- Capability matrix
- Permissions
- Scheduler
- Backoff
- Health
- Cancellation
- Last-good cache
- Contract tests

Exit gate:

- Fake provider demonstrates success, partial, rate limit and crash
- One provider failure does not affect another

## Milestone 3 — First Collection Modes

Deliverable: End-to-end proof for all collection modes

- Local JSONL adapter
- SQLite reader adapter
- Hook manager
- Official API adapter
- Ollama local API
- Browser Native Messaging
- Privacy validation

Exit gate:

- Local log, Hook, API, Browser and Local API each produce normalized data

## Milestone 4 — Provider Set v0.1

Deliverable: Ten provider groups

Recommended implementation order:

1. Claude
2. Codex/OpenAI
3. OpenCode
4. Ollama
5. OpenRouter
6. Gemini
7. Cursor
8. Copilot
9. Grok
10. Kiro

Each provider receives its own Task, fixtures, permissions, capability docs and review

## Milestone 5 — Analytics and Pricing

Deliverable: Full local analytics

- Rollups
- Time range queries
- Heatmap
- Pricing catalog
- Overrides
- Cost calculation
- Budget
- Forecast
- Export
- Retention

Exit gate:

- Actual and estimated costs are distinguishable
- Recalculate history is explicit
- Export passes privacy scan

## Milestone 6 — Desktop UX

Deliverable: Main Dashboard, Tray Popup and Floating Widget

- Overview
- Providers
- Analytics
- Costs
- Budgets
- Alerts
- Settings
- System
- Tray
- Floating widget
- Dark/light themes
- Accessibility

Exit gate:

- Loading/empty/stale/partial/error states
- Position recovery
- Shared read model
- Keyboard navigation

## Milestone 7 — Community Adapter Sandbox

Deliverable: Permissioned Wasm adapter proof

- Manifest schema
- Package validation
- Wasm runtime
- Capability handles
- Permission UI
- Install/disable/remove
- Sample adapter
- Crash limits

Exit gate:

- Adapter cannot access undeclared file/network
- Crash does not affect app
- Permission revocation takes effect

## Milestone 8 — Windows Packaging and Update

Deliverable: Installable releases

- Per-user NSIS
- Portable
- x64/ARM64/x86 matrix
- WebView2 detection
- Chrome/Edge Native Messaging registration
- Signed updater
- Background download
- Restart prompt
- Checksums/SBOM/provenance

Exit gate:

- Tier 1 release pipeline passes
- x86 compatibility report generated
- Invalid update signature is rejected

## Milestone 9 — Release hardening

- Performance
- Security audit
- Accessibility audit
- Migration rehearsal
- Upgrade/downgrade behavior
- Documentation
- Contributor templates
- Release notes
- Clean-machine smoke tests

## Post-v0.1 candidates

- macOS
- Linux
- Optional encrypted database
- Optional local network aggregation
- Signed community registry
- More providers
- Multi-currency conversion
- User-defined dashboard layouts
- Import from other trackers
