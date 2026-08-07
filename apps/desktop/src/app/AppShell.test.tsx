import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { AppShell } from "./AppShell";
import * as native from "../lib/native";

vi.mock("../lib/native", async (importOriginal) => {
  const actual = await importOriginal<typeof native>();
  return {
    ...actual,
    fetchAlerts: vi.fn(),
    fetchSettings: vi.fn(),
    fetchPipelineDiagnostics: vi.fn(),
    refreshAll: vi.fn(),
  };
});

const diagnostics = (
  lastSuccessfulSync: string | null,
): native.PipelineDiagnostics => ({
  app_version: "0.2.0",
  db_ok: true,
  integrity_ok: true,
  migration_version: 6,
  total_events: 0,
  totals: {
    events_seen: 0,
    events_parsed: 0,
    events_normalized: 0,
    events_rejected: 0,
    duplicates_skipped: 0,
    events_inserted: 0,
    quota_snapshots_inserted: 0,
    privacy_rejections: 0,
    last_successful_sync: lastSuccessfulSync,
    next_retry_at: null,
  },
  providers: [],
  runs: [],
});

const settingsView = (theme: "dark" | "light" | "system"): native.SettingsViewData => ({
  settings: {
    launch_at_startup: false,
    theme,
    refresh_interval_seconds: 300,
    auto_update_check: true,
    widget_opacity: 1,
    widget_locked: false,
    widget_visible: false,
    widget_size: "medium",
    retention_days: 90,
    pet_visible: false,
    pet_character: "robot",
    pet_speed: "normal",
    pet_opacity: 1,
    pet_auto_sleep: true,
  },
  startup_supported: true,
  startup_registered: false,
  credential_store_supported: true,
  provider_credentials: [],
  allowed_refresh_intervals: [0, 300],
  allowed_themes: ["dark", "light", "system"],
  allowed_retention_days: [90],
});

const alerts = (openCount: number): native.AlertsViewData => ({
  generated_at: "2026-08-04T00:00:00Z",
  open: [],
  history: [],
  open_count: openCount,
  critical_count: 0,
  unacknowledged_count: 0,
});

function renderShell() {
  return render(
    <MemoryRouter initialEntries={["/"]}>
      <AppShell />
    </MemoryRouter>,
  );
}

describe("AppShell", () => {
  beforeEach(() => {
    vi.mocked(native.fetchAlerts).mockResolvedValue(alerts(0));
    vi.mocked(native.fetchSettings).mockResolvedValue(settingsView("dark"));
    vi.mocked(native.fetchPipelineDiagnostics).mockResolvedValue(
      diagnostics(null),
    );
    vi.mocked(native.refreshAll).mockReset();
  });

  it("states that nothing has been collected instead of showing a fresh badge", async () => {
    renderShell();
    await waitFor(() =>
      expect(
        screen.getByText("No collection has succeeded yet"),
      ).toBeInTheDocument(),
    );
    expect(screen.getByText("No data")).toBeInTheDocument();
    expect(screen.queryByText("Fresh")).not.toBeInTheDocument();
  });

  it("reports freshness from the last successful collection", async () => {
    vi.mocked(native.fetchPipelineDiagnostics).mockResolvedValue(
      diagnostics(new Date(Date.now() - 60_000).toISOString()),
    );
    renderShell();
    await waitFor(() => expect(screen.getByText("Fresh")).toBeInTheDocument());
    expect(screen.getByText(/Collected 1 min ago/)).toBeInTheDocument();
  });

  it("marks old data as stale rather than fresh", async () => {
    vi.mocked(native.fetchPipelineDiagnostics).mockResolvedValue(
      diagnostics(new Date(Date.now() - 60 * 60_000).toISOString()),
    );
    renderShell();
    await waitFor(() => expect(screen.getByText("Stale")).toBeInTheDocument());
  });

  it("surfaces a failed refresh instead of swallowing it", async () => {
    vi.mocked(native.refreshAll).mockRejectedValue(
      new Error("storage not initialized"),
    );
    renderShell();
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Refresh all providers" }),
      ).toBeEnabled(),
    );
    await userEvent.click(
      screen.getByRole("button", { name: "Refresh all providers" }),
    );
    await waitFor(() =>
      expect(
        screen.getByText("Refresh failed: storage not initialized"),
      ).toBeInTheDocument(),
    );
  });

  it("applies the stored theme to the document", async () => {
    vi.mocked(native.fetchSettings).mockResolvedValue(settingsView("light"));
    renderShell();
    await waitFor(() =>
      expect(document.documentElement.dataset.theme).toBe("light"),
    );
  });

  it("shows the open alert count in the navigation", async () => {
    vi.mocked(native.fetchAlerts).mockResolvedValue(alerts(3));
    renderShell();
    await waitFor(() =>
      expect(screen.getByLabelText("3 open alerts")).toBeInTheDocument(),
    );
  });
});
