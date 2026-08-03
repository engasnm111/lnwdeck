# Contributing to inwdeck

Thank you for your interest in contributing to inwdeck! Please review the following documents before getting started:

- `AGENTS.md` — Mandatory rules for AI agents and human contributors
- `docs/00_PROJECT_CHARTER.md` — Vision, scope, and locked decisions
- `docs/02_SYSTEM_ARCHITECTURE.md` — Architecture and layer boundaries
- `docs/05_SECURITY_PRIVACY.md` — Privacy, secrets, hook, and browser helper specifications
- `docs/08_TESTING_QA.md` — Testing strategy and quality gates

## Working Principles

- Complete one task at a time from the Implementation Plan.
- Commits must be small, readable, and focused on a single logical purpose.
- Do not modify requirements independently; do not start the next task until the current task is reviewed and approved.
- Execute work inline; do not spawn subagents unless explicitly authorized.

## Privacy-First Rule

- Store metadata only.
- Never store prompts, responses, source code, file names, or absolute paths.
- All new data fields must pass fail-closed privacy guards.
- Test fixtures must consist strictly of synthetic data with no real user information.

## Pull Request Quality Gates

The following checks must pass before merging a PR:

- `pnpm check`
- `pnpm test`
- `cargo test --workspace --all-features`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --check`
- If modifying providers: run provider contract test suite + privacy scan

## Community / Verified Adapters

- **Built-in**: Maintainer code review + full contract test suite.
- **Verified Community**: Public source repository, verified checksum, code review, passing contract tests.
- **Unverified Community**: Requires user warning prompt and manual installation approval.
- Execute community adapter code inside Wasm sandboxes; native DLL plugins are forbidden.

## Dependencies

- New dependencies must state a valid justification, undergo license checks, and evaluate size impact.
- Never load fonts, scripts, or UI assets from external CDNs at runtime.

## Bug Fixes

- Reproduce issues before fixing; implement isolated changes without breaking other provider integrations.
- Errors must use typed error enums with sanitized contexts that do not expose sensitive data.
- Avoid `unwrap()` and `expect()` in production execution paths.

## Commit Messages

Follow Conventional Commits (e.g., `feat:`, `fix:`, `chore:`, `docs:`, `test:`, `build:`).

Example from implementation roadmap:

```text
chore: establish inwdeck workspace
feat: expose secure desktop application commands
```

## Pull Request Workflow

1. Create a feature branch off `main` or `dev`.
2. Ensure all quality gate checks pass locally.
3. Open a Pull Request using `.github/PULL_REQUEST_TEMPLATE.md`.
4. Await reviewer feedback, approval, and merge.