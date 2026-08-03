# AGENTS.md — lnwdeck Engineering Rules

This document outlines mandatory rules for all AI coding agents and human contributors.

## 1. Workflow Guidelines

1. Read documentation in `docs/` before modifying code.
2. Complete one task at a time from the Implementation Plan.
3. Do not create subagents unless explicitly instructed by the user; execute inline by default.
4. Before modifying files, summarize the task scope and the list of files to be created or modified.
5. Practice Test-Driven Development (TDD):
   - Write a failing test first.
   - Run to confirm failure for the expected reason.
   - Write minimum necessary implementation code.
   - Run relevant tests.
   - Run workspace quality gates.
6. Stop and report when a task passes its Acceptance Criteria. Do not automatically proceed to the next task.
7. Commits must be small, readable, and focused on a single logical purpose.
8. Never use placeholders or empty function implementations, and never disable tests to make CI pass.

## 2. Immutable Product Scope Constraints

- Product name: `lnwdeck`
- Local-only by default in v0.1; no cloud accounts or cloud sync.
- Store metadata only.
- Never store prompts, responses, source code, file contents, file names, or absolute paths.
- Project identity must use aliases and keyed hash identifiers.
- Hooks must start in passive mode and require user approval before installation.
- All configuration modifications must support preview, backup, validation, and rollback.
- Browser Helper must never transmit cookies or session tokens out of the browser.
- Community Adapters must run in sandboxes with deny-by-default permissions.
- x64 and ARM64 are Tier 1 architectures; x86 is a Compatibility Tier.
- Minimum supported Windows OS version: Windows 10 22H2.

## 3. Architecture Boundaries

- `crates/domain`: Domain types and invariants only.
- `crates/application`: Use cases and orchestration logic.
- `crates/storage`: SQLite, migrations, and repositories.
- `crates/security`: Credentials, hashing, redaction, and permission checks.
- `crates/provider-runtime`: Adapter lifecycle, scheduling, and sandbox isolation.
- `crates/providers/*`: Built-in provider adapters.
- `apps/desktop/src`: React UI application.
- `apps/desktop/src-tauri`: Tauri commands and native OS integration.
- `apps/browser-extension`: Chromium Manifest V3 extension.
- `schemas`: JSON Schema and WIT contracts as the single source of truth.
- UI must never query SQLite directly; requests must pass through Tauri commands/use cases.
- Provider adapters must never write to the database directly; they must return normalized batches to Core.
- Secrets must never flow into the UI, logs, analytics events, or unauthorized adapters.

## 4. Security Rules

- Deny-by-default policy for File, Network, Credential, and Hook permissions.
- Use Windows Credential Manager or Tauri Stronghold based on threat modeling.
- Never store secrets in `.env`, JSON files, SQLite, or source code.
- Logs must pass through redaction before being written to disk.
- Native Messaging must validate extension origin against an allowlist.
- Network destinations must match domains declared in the adapter manifest.
- HTTPS only, except for loopback/local providers explicitly enabled by the user.
- Database migrations must be transactional and backed up beforehand.
- Update artifacts must verify cryptographic signatures before installation.
- New dependencies must provide explicit justification and undergo license verification.

## 5. Coding Standards

### Rust

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- Avoid `unwrap()` and `expect()` in production execution paths.
- Errors must use typed error enums with sanitized contexts that do not expose sensitive data.
- Public APIs must include Rustdoc documentation.
- All async tasks must be cancellable and enforce timeout limits.

### TypeScript / React

- TypeScript strict mode enforced.
- No `any` types unless accompanied by an explanatory comment and runtime validation.
- Components must adhere to the Single Responsibility Principle.
- Data received from Native/Extension layers must be validated against schemas.
- Avoid duplicate state and persistent derived state.
- UI state must handle Loading, Empty, Stale, Partial, and Error states gracefully.
- Accessibility: Keyboard navigation, visible focus indicators, semantic roles, reduced motion support.

### SQL

- Database migrations are append-only.
- Never modify existing released migrations.
- Critical queries must include index execution plans and integration tests.
- Timestamps must be stored in UTC.
- Monetary amounts must be stored as integer minor units or decimal strings (never floating-point).

## 6. Testing Requirements

- Domain logic: Unit tests.
- Storage: Migration + repository integration tests.
- Adapters: Contract tests + sanitized fixtures.
- Tauri commands: Integration tests.
- UI: Vitest + React Testing Library.
- Main workflow: Playwright E2E tests.
- Browser Helper: Extension unit tests + Native Messaging protocol tests.
- Privacy: Fixture scans must verify that prompts, responses, and paths are not persisted.
- Release: Smoke tests for Installer and Portable builds on x64; automated build checks required for Tier 1 architectures.

## 7. File Safety Guidelines

- Do not modify files outside the current task scope without justification.
- Do not overwrite existing user configuration.
- Do not overwrite pre-existing system hooks; if chain integration is not possible, stop and notify the user.
- Generated files must be marked as generated and never edited manually.
- Test fixtures must consist strictly of synthetic data with no real user information.
- External assets required for offline UI must be vendored in the repository with proper licenses.
- Never load fonts, scripts, or UI assets from external CDNs at runtime.

## 8. Completion Report

Upon completing a task, provide a report covering:

1. Modified files
2. Added or updated behavior
3. Tests and verification commands executed, including results
4. Remaining limitations or constraints
5. Security/Privacy impact
6. Commit hash (if applicable)
7. Confirmation that the next task was not executed automatically
