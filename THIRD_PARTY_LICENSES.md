# Third-Party Licenses

This document lists third-party open source components and their licenses used
by `inwdeck`. It is maintained as work proceeds; each dependency added in a Task
must be recorded here with its license and a short justification.

Dependency license scanning runs in CI and the release is blocked when a
license policy failure exists.

## Requested as inspiration / reference (not vendored)

| Project | Reference | Status |
|---|---|---|
| TokenTracker | https://github.com/xiufengsun/TokenTracker | Studied for collection techniques; MIT-compatible. No code is reused without recorded attribution and license confirmation. |

## Embedded / vendored assets

- _(None yet — v0.1 ships no third-party source code or vendored assets)_

## Policy

- New dependency must include a reason in its PR and this list.
- Code reuse from third-party sources follows the rules in
  `docs/09_OPEN_SOURCE_GOVERNANCE.md` (confirm license, record source commit,
  preserve notices, adapt to inwdeck privacy rules, add tests).
- Assets that must be present for offline UI are stored in-repo with a
  compatible license and never loaded from a CDN at runtime.