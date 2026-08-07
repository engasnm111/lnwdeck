import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { AlertsPage } from "./AlertsPage";
import * as native from "../../lib/native";

vi.mock("../../lib/native", async (importOriginal) => {
  const actual = await importOriginal<typeof native>();
  return { ...actual, fetchAlerts: vi.fn(), acknowledgeAlert: vi.fn() };
});

const alert = (
  overrides: Partial<native.AlertRowData> = {},
): native.AlertRowData => ({
  id: 1,
  alert_key: "quota:openrouter_api:credits",
  kind: "quota_threshold",
  severity: "critical",
  provider_id: "openrouter_api",
  title: "OpenRouter Credits window at 4% remaining",
  detail: "used 9600000 of 10000000",
  error_code: "",
  first_seen_at: "2026-08-04T00:00:00Z",
  last_seen_at: "2026-08-04T01:00:00Z",
  occurrences: 3,
  acknowledged_at: null,
  resolved_at: null,
  ...overrides,
});

describe("AlertsPage", () => {
  beforeEach(() => {
    vi.mocked(native.fetchAlerts).mockReset();
    vi.mocked(native.acknowledgeAlert).mockReset();
  });

  it("says no alerts are open without claiming the system is healthy", async () => {
    vi.mocked(native.fetchAlerts).mockResolvedValue({
      generated_at: "2026-08-04T00:00:00Z",
      open: [],
      history: [],
      open_count: 0,
      critical_count: 0,
      unacknowledged_count: 0,
    });
    render(<AlertsPage />);

    await waitFor(() =>
      expect(screen.getByText(/No alerts are open/i)).toBeInTheDocument(),
    );
    expect(screen.queryByText(/All Systems Normal/i)).not.toBeInTheDocument();
  });

  it("renders an open alert with its severity and occurrences", async () => {
    vi.mocked(native.fetchAlerts).mockResolvedValue({
      generated_at: "2026-08-04T00:00:00Z",
      open: [alert()],
      history: [alert()],
      open_count: 1,
      critical_count: 1,
      unacknowledged_count: 1,
    });
    render(<AlertsPage />);

    await waitFor(() =>
      expect(
        screen.getByText("OpenRouter Credits window at 4% remaining"),
      ).toBeInTheDocument(),
    );
    expect(screen.getAllByText("Critical").length).toBeGreaterThan(0);
    expect(screen.getByText(/3 occurrence/)).toBeInTheDocument();
    expect(screen.getByText("Needs attention")).toBeInTheDocument();
  });

  it("acknowledges an alert through the backend", async () => {
    vi.mocked(native.fetchAlerts).mockResolvedValue({
      generated_at: "2026-08-04T00:00:00Z",
      open: [alert()],
      history: [],
      open_count: 1,
      critical_count: 1,
      unacknowledged_count: 1,
    });
    vi.mocked(native.acknowledgeAlert).mockResolvedValue(undefined);
    render(<AlertsPage />);

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Acknowledge" }),
      ).toBeInTheDocument(),
    );
    await userEvent.click(screen.getByRole("button", { name: "Acknowledge" }));
    expect(native.acknowledgeAlert).toHaveBeenCalledWith(1);
  });

  it("shows a failed acknowledgement instead of hiding it", async () => {
    vi.mocked(native.fetchAlerts).mockResolvedValue({
      generated_at: "2026-08-04T00:00:00Z",
      open: [alert()],
      history: [],
      open_count: 1,
      critical_count: 1,
      unacknowledged_count: 1,
    });
    vi.mocked(native.acknowledgeAlert).mockRejectedValue(
      new Error("alert 1 is unknown or already acknowledged"),
    );
    render(<AlertsPage />);

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Acknowledge" }),
      ).toBeInTheDocument(),
    );
    await userEvent.click(screen.getByRole("button", { name: "Acknowledge" }));

    await waitFor(() =>
      expect(
        screen.getByText("alert 1 is unknown or already acknowledged"),
      ).toBeInTheDocument(),
    );
  });
});
