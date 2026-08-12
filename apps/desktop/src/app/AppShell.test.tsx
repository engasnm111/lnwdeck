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
    fetchAppFreshness: vi.fn(),
    fetchAppShellStatus: vi.fn(),
    startRefresh: vi.fn(),
  };
});

const shellStatus = (
  lastSuccessfulSync: string | null,
  theme: "dark" | "light" | "system" = "dark",
  unacknowledgedAlertCount = 0,
) => ({
  app_version: "0.2.0",
  last_successful_sync: lastSuccessfulSync,
  theme,
  unacknowledged_alert_count: unacknowledgedAlertCount,
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
    vi.mocked(native.fetchAlerts).mockReset();
    vi.mocked(native.fetchSettings).mockReset();
    vi.mocked(native.fetchAppFreshness).mockReset();
    vi.mocked(native.fetchAppShellStatus).mockReset();
    vi.mocked(native.fetchAppShellStatus).mockResolvedValue(shellStatus(null));
    vi.mocked(native.startRefresh).mockReset();
  });

  it("loads topbar, theme and alert metadata through one lightweight shell request", async () => {
    renderShell();

    await waitFor(() =>
      expect(native.fetchAppShellStatus).toHaveBeenCalledTimes(1),
    );
    expect(native.fetchAlerts).not.toHaveBeenCalled();
    expect(native.fetchSettings).not.toHaveBeenCalled();
    expect(native.fetchAppFreshness).not.toHaveBeenCalled();
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
    vi.mocked(native.fetchAppShellStatus).mockResolvedValue(
      shellStatus(new Date(Date.now() - 60_000).toISOString()),
    );
    renderShell();
    await waitFor(() => expect(screen.getByText("Fresh")).toBeInTheDocument());
    expect(screen.getByText(/Collected 1 minute ago/)).toBeInTheDocument();
  });

  it("marks old data as stale rather than fresh", async () => {
    vi.mocked(native.fetchAppShellStatus).mockResolvedValue(
      shellStatus(new Date(Date.now() - 60 * 60_000).toISOString()),
    );
    renderShell();
    await waitFor(() => expect(screen.getByText("Stale")).toBeInTheDocument());
  });

  it("surfaces a failed refresh instead of swallowing it", async () => {
    vi.mocked(native.startRefresh).mockRejectedValue(
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
    expect(
      screen.getByRole("button", { name: "Refresh all providers" }),
    ).toBeEnabled();
  });

  it("keeps the refresh state when another surface already owns the shared job", async () => {
    vi.mocked(native.startRefresh).mockResolvedValue({
      started: false,
      already_running: true,
    });
    renderShell();
    const refreshButton = await screen.findByRole("button", {
      name: "Refresh all providers",
    });

    await userEvent.click(refreshButton);

    await waitFor(() => expect(native.startRefresh).toHaveBeenCalledTimes(1));
    expect(refreshButton).toBeDisabled();
  });

  it("applies the stored theme to the document", async () => {
    vi.mocked(native.fetchAppShellStatus).mockResolvedValue(
      shellStatus(null, "light"),
    );
    renderShell();
    await waitFor(() =>
      expect(document.documentElement.dataset.theme).toBe("light"),
    );
  });

  it("shows the open alert count in the navigation", async () => {
    vi.mocked(native.fetchAppShellStatus).mockResolvedValue(
      shellStatus(null, "dark", 3),
    );
    renderShell();
    await waitFor(() =>
      expect(
        screen.getByLabelText("3 unacknowledged alerts"),
      ).toBeInTheDocument(),
    );
  });

  it("removes the alert badge when another page acknowledges an alert", async () => {
    vi.mocked(native.fetchAppShellStatus)
      .mockResolvedValueOnce(shellStatus(null, "dark", 3))
      .mockResolvedValueOnce(shellStatus(null, "dark", 0));
    renderShell();

    await waitFor(() =>
      expect(
        screen.getByLabelText("3 unacknowledged alerts"),
      ).toBeInTheDocument(),
    );

    window.dispatchEvent(new Event("lnwdeck:alerts-updated"));

    await waitFor(() =>
      expect(
        screen.queryByLabelText("3 unacknowledged alerts"),
      ).not.toBeInTheDocument(),
    );
    expect(native.fetchAppShellStatus).toHaveBeenCalledTimes(2);
  });
});
