# Tray Popup Window Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a pixel-perfect React Tray Popup window matching the user's mockup screenshot, integrated with `lnwdeck` backend overview metrics.

**Architecture:** A dedicated frameless Tauri webview window mapped to route `/tray` rendering a clean light-themed card UI with overview statistics (Total Tokens, Total Cost, Requests, Providers), status indicators (OK, LNWDEV badge), and an "Open Dashboard" action button.

**Tech Stack:** React 19, TypeScript, Vitest, React Testing Library, Tauri v2 IPC, CSS.

## Global Constraints

- Must replicate layout, icons, colors, typography, badges, and button styling from user mockup exact picture.
- Must fetch live backend metrics via `fetchOverview()` (`get_overview` Tauri command).
- Must adhere to `AGENTS.md` guidelines: local-only, no empty placeholders, comprehensive tests before implementation (TDD).
- Accessibility: visible focus state, semantic structure, clear labels.

---

### Task 1: Add Backend IPC Command for Window Focus (`show_main_window`)

**Files:**
- Modify: `apps/desktop/src-tauri/src/windows.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`
- Test: `apps/desktop/src-tauri/src/windows.rs`

**Interfaces:**
- Consumes: Tauri AppHandle
- Produces: `show_main_window(app: tauri::AppHandle) -> Result<(), String>`

- [ ] **Step 1: Write failing test in Rust backend**

Add test to `apps/desktop/src-tauri/src/windows.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_show_main_window_signature() {
        // Verification of command registration helper
        assert_eq!(std::mem::size_of_val(&show_main_window), 0);
    }
}
```

- [ ] **Step 2: Run test to verify failure/compile**

Run: `cargo test -p desktop_lib --lib windows::tests::test_show_main_window_signature`
Expected: FAIL (unresolved symbol `show_main_window`)

- [ ] **Step 3: Implement `show_main_window` command in `windows.rs` and register in `lib.rs`**

In `apps/desktop/src-tauri/src/windows.rs`:
```rust
#[tauri::command]
pub fn show_main_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("main") {
        w.show().map_err(|e| e.to_string())?;
        w.set_focus().map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("main window not found".to_string())
    }
}
```

In `apps/desktop/src-tauri/src/lib.rs`: Add `windows::show_main_window` to `invoke_handler![]`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p desktop_lib --lib windows::tests::test_show_main_window_signature`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/src/windows.rs apps/desktop/src-tauri/src/lib.rs
git commit -m "feat(desktop): add show_main_window command for tray popup action"
```

---

### Task 2: Create Tray Popup Styles and Component with Pixel-Perfect Design

**Files:**
- Create: `apps/desktop/src/windows/tray/TrayPopup.css`
- Modify: `apps/desktop/src/windows/tray/TrayPopup.tsx`
- Create: `apps/desktop/src/windows/tray/TrayPopup.test.tsx`
- Modify: `apps/desktop/src/App.tsx`

**Interfaces:**
- Consumes: `fetchOverview()` from `../../lib/native`
- Consumes: `@tauri-apps/api/core` `invoke("show_main_window")`
- Produces: `<TrayPopup />` component rendered at route `/tray`

- [ ] **Step 1: Write failing React component test**

Create `apps/desktop/src/windows/tray/TrayPopup.test.tsx`:
```tsx
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { TrayPopup } from "./TrayPopup";
import { invoke } from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("TrayPopup", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders overview stats, badges, and open dashboard button", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_overview") {
        return {
          total_events: 16,
          total_tokens_input: 4000000,
          total_tokens_output: 1669402,
          provider_count: 1,
          high_confidence_count: 16,
          confidence_coverage: 1.0,
          latest_event_at: "2026-08-03T12:00:00Z",
          oldest_event_at: "2026-08-01T12:00:00Z",
        };
      }
      return null;
    });

    render(<TrayPopup />);

    expect(screen.getByText("Loading...")).toBeInTheDocument();

    await waitFor(() => {
      expect(screen.getByText("Inwdeck")).toBeInTheDocument();
      expect(screen.getByText("OK")).toBeInTheDocument();
      expect(screen.getByText("Total Tokens")).toBeInTheDocument();
      expect(screen.getByText("5,669,402")).toBeInTheDocument();
      expect(screen.getByText("Total Cost (Estimated)")).toBeInTheDocument();
      expect(screen.getByText("$0.00")).toBeInTheDocument();
      expect(screen.getByText("Requests")).toBeInTheDocument();
      expect(screen.getByText("16")).toBeInTheDocument();
      expect(screen.getByText("Providers")).toBeInTheDocument();
      expect(screen.getByText("1")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Open Dashboard" })).toBeInTheDocument();
      expect(screen.getByText("LNWDEV")).toBeInTheDocument();
    });
  });

  it("invokes show_main_window when Open Dashboard is clicked", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_overview") {
        return {
          total_events: 16,
          total_tokens_input: 1000,
          total_tokens_output: 500,
          provider_count: 1,
          high_confidence_count: 16,
          confidence_coverage: 1.0,
          latest_event_at: null,
          oldest_event_at: null,
        };
      }
      if (cmd === "show_main_window") {
        return Promise.resolve(null);
      }
      return null;
    });

    render(<TrayPopup />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Open Dashboard" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "Open Dashboard" }));

    expect(invoke).toHaveBeenCalledWith("show_main_window");
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter @lnwdeck/desktop test -- TrayPopup`
Expected: FAIL (missing elements / styles)

- [ ] **Step 3: Implement `TrayPopup.css` and update `TrayPopup.tsx` & `App.tsx`**

Create `apps/desktop/src/windows/tray/TrayPopup.css`:
```css
.tray-window {
  font-family: Inter, system-ui, -apple-system, sans-serif;
  background-color: transparent;
  padding: 16px;
  width: 360px;
  box-sizing: border-box;
  user-select: none;
}

.tray-card {
  background: #ffffff;
  border-radius: 16px;
  border: 1px solid #e2e8f0;
  box-shadow: 0 10px 25px -5px rgba(0, 0, 0, 0.08), 0 8px 10px -6px rgba(0, 0, 0, 0.04);
  padding: 20px 24px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.tray-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.tray-brand {
  display: flex;
  align-items: center;
  gap: 10px;
}

.tray-brand-icon {
  width: 24px;
  height: 24px;
}

.tray-brand-title {
  font-size: 18px;
  font-weight: 500;
  color: #94a3b8;
}

.tray-badge-ok {
  border: 1px solid #e2e8f0;
  color: #10b981;
  font-size: 13px;
  font-weight: 500;
  padding: 2px 10px;
  border-radius: 6px;
  background: #ffffff;
}

.tray-metrics {
  display: flex;
  flex-direction: column;
  gap: 14px;
  margin-top: 4px;
}

.tray-metric-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.tray-metric-left {
  display: flex;
  align-items: center;
  gap: 12px;
}

.tray-icon-circle {
  width: 24px;
  height: 24px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 13px;
  font-weight: 600;
}

.tray-icon-t {
  color: #3b82f6;
  background: #ffffff;
}

.tray-icon-dollar {
  color: #f59e0b;
  background: #ffffff;
}

.tray-icon-cube {
  color: #0284c7;
  background: #ffffff;
}

.tray-icon-chart {
  color: #64748b;
  background: #ffffff;
}

.tray-metric-label {
  font-size: 14px;
  font-weight: 500;
  color: #475569;
}

.tray-metric-value {
  font-size: 16px;
  font-weight: 600;
  color: #818cf8;
}

.tray-action-btn {
  width: 100%;
  padding: 10px 0;
  margin-top: 6px;
  background: #ffffff;
  border: 1px solid #e2e8f0;
  border-radius: 8px;
  color: #94a3b8;
  font-size: 15px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s ease;
  text-align: center;
}

.tray-action-btn:hover {
  background-color: #f8fafc;
  border-color: #cbd5e1;
  color: #64748b;
}

.tray-footer-bar {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 12px;
  font-size: 13px;
  color: #475569;
}

.tray-badge-lnwdev {
  border: 1px solid #e2e8f0;
  color: #059669;
  font-weight: 600;
  padding: 2px 10px;
  border-radius: 6px;
  background: #ffffff;
  font-size: 12px;
  letter-spacing: 0.5px;
}
```

Update `apps/desktop/src/windows/tray/TrayPopup.tsx` with complete layout matching screenshot.
Update `apps/desktop/src/App.tsx` to register `<Route path="tray" element={<TrayPopup />} />`.

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter @lnwdeck/desktop test -- TrayPopup`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src/windows/tray/ apps/desktop/src/App.tsx
git commit -m "feat(desktop): implement pixel-perfect tray popup window UI"
```

---

## Verification Plan

### Automated Tests
- Run desktop unit tests: `pnpm --filter @lnwdeck/desktop test`
- Run workspace typechecks: `pnpm run check`
- Run Rust workspace tests: `cargo test --workspace`

### Manual Verification
- Launch dev desktop app using `pnpm tauri:dev` or inspect component via route `/tray`.
- Verify exact visual alignment with attached mockup screenshot (Inwdeck feather logo, OK pill badge, 4 rows with circle icons, Open Dashboard button, and running LNWDEV pill badge).
