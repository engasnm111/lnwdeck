# Pet Tooltip and Widget Filtering Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Compact the desktop-pet bubbles and show only successfully fetched quota providers in the floating widget.

**Architecture:** Keep sizing in the existing pet CSS so the browser performs intrinsic layout without Tauri resize calls. Reuse `hasFetchedQuota` as the single widget availability predicate, then apply the saved provider selection to that filtered list.

**Tech Stack:** React, TypeScript, CSS, Vitest, React Testing Library, Tauri 2

## Global Constraints

- Quota row gaps must not exceed 20px.
- Tooltip and speech borders must remain inside the WebView viewport.
- Failed quota providers must not render in any widget selection mode.
- Known providers remain available in the picker for recovery.
- Do not persist or expose credentials or account identifiers.

---

### Task 1: Lock failed-provider filtering with a regression

**Files:**
- Modify: `apps/desktop/src/windows/widget/FloatingWidget.test.tsx`

**Interfaces:**
- Consumes: `FloatingWidget`, `fetchWidgetSettings`, `fetchQuotaDashboard`
- Produces: component-level regression for explicit selections

- [ ] **Step 1: Write the failing test**

Add a test with `selected_providers: ["shown", "failed"]`, one fresh provider
with a quota window, and one `auth_expired` provider without windows. Assert
that `Shown` renders, `Failed` does not, and the footer reports one visible
provider.

- [ ] **Step 2: Run test to verify it fails**

Run:

```powershell
pnpm --filter @lnwdeck/desktop test -- FloatingWidget.test.tsx -t "hides an explicitly selected provider whose quota fetch failed"
```

Expected: FAIL because the selected-provider branch currently filters
`allProviders` instead of `fetchedProviders`.

### Task 2: Apply availability before selection

**Files:**
- Modify: `apps/desktop/src/windows/widget/FloatingWidget.tsx`
- Test: `apps/desktop/src/windows/widget/FloatingWidget.test.tsx`

**Interfaces:**
- Consumes: `hasFetchedQuota`, `fetchedProviders`, `settings.selected_providers`
- Produces: `visibleProviders` containing only fetched selected providers

- [ ] **Step 1: Implement the minimal logic change**

When a selection exists, filter `fetchedProviders` by the selected provider
ids. Never return directly from `allProviders`.

- [ ] **Step 2: Render the correct empty state**

When no renderable selected provider remains, use the localized no-quota state
instead of rendering an unavailable card.

- [ ] **Step 3: Run the focused test**

Run the Task 1 command and expect PASS.

### Task 3: Make quota and speech bubbles content-sized

**Files:**
- Modify: `apps/desktop/src/windows/pet/DesktopPet.css`

**Interfaces:**
- Consumes: existing `.pet-tooltip-inner`, `.pet-tooltip-bar-row`, and `.pet-tooltip-speech` markup
- Produces: intrinsic-width quota and speech layouts bounded by the viewport

- [ ] **Step 1: Compact the quota bubble**

Use intrinsic width on `.pet-tooltip-inner`, a bounded label column, fixed bar
and percentage columns, and a 12px row gap.

- [ ] **Step 2: Compact speech independently**

Override speech to use max-content width capped at 360px and viewport minus
16px, retaining two-line wrapping.

- [ ] **Step 3: Preserve the frame**

Keep viewport-aware max width, border-box sizing, vertical overflow, border,
radius, shadow, and arrow unchanged.

### Task 4: Verify and publish

**Files:**
- Modify: `Cargo.lock`
- Modify: `apps/desktop/src-tauri/Cargo.toml`
- Modify: `apps/desktop/src-tauri/tauri.conf.json`
- Modify: `installer/package-config.json`
- Create: `docs/releases/v11.0.7.md`

**Interfaces:**
- Consumes: release workflow version contract
- Produces: signed x64, ARM64, and x86 v11.0.7 artifacts

- [ ] **Step 1: Run focused and workspace verification**

Run focused widget/pet tests, `pnpm check`, `cargo fmt --check`, `cargo clippy`,
workspace tests, and a Tauri debug build.

- [ ] **Step 2: Inspect the real pet window**

Confirm the quota label-to-bar gap is at most 20px, short speech hugs its text,
and all glass-frame edges remain visible.

- [ ] **Step 3: Bump release metadata to 11.0.7**

Update the three declared versions, package artifact names, lockfile package
version, and release notes.

- [ ] **Step 4: Commit and publish**

Commit the scoped files, push `main`, wait for CI and Security to pass, create
and push `v11.0.7`, then wait for the Release workflow and verify all assets.
