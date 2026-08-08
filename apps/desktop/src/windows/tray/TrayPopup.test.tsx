import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { TrayPopup } from "./TrayPopup";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { I18nProvider } from "../../app/I18nProvider";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(),
}));

type Handler = (event: { payload: unknown }) => void;

/** Captures the event handlers the tray popup registers. */
function captureListeners() {
  const handlers = new Map<string, Handler>();
  vi.mocked(listen).mockImplementation(
    async (event: string, handler: unknown) => {
      handlers.set(event, handler as Handler);
      return () => {};
    },
  );
  return handlers;
}

describe("TrayPopup Component", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(listen).mockResolvedValue(() => {});
  });

  it("renders loading state initially and then overview data", async () => {
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

    render(
      <I18nProvider>
        <TrayPopup />
      </I18nProvider>,
    );

    await waitFor(() => {
      expect(document.querySelector(".tray-window")).toHaveAttribute(
        "data-surface",
        "opaque",
      );
      expect(screen.getByText("lnwdeck")).toBeInTheDocument();
      expect(screen.getByText("OK")).toBeInTheDocument();
      expect(screen.getByText("Total tokens")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Total tokens: 5,669,402" })).toBeInTheDocument();
      expect(screen.getByText("Estimated cost")).toBeInTheDocument();
      expect(screen.getByText("Not available")).toBeInTheDocument();
      expect(screen.queryByText("$0.00")).not.toBeInTheDocument();
      expect(screen.getByText("Requests")).toBeInTheDocument();
      expect(screen.getByText("16")).toBeInTheDocument();
      expect(screen.getByText("Providers")).toBeInTheDocument();
      expect(screen.getByText("1")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Open dashboard" })).toBeInTheDocument();
      expect(screen.getByText("LNWDEV")).toBeInTheDocument();
      expect(screen.getByText("Running")).toBeInTheDocument();
    });
  });

  it("opens the dashboard through the tray navigation command", async () => {
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
      if (cmd === "open_dashboard_from_tray") {
        return Promise.resolve(null);
      }
      return null;
    });

    render(
      <I18nProvider>
        <TrayPopup />
      </I18nProvider>,
    );

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Open dashboard" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "Open dashboard" }));

    expect(invoke).toHaveBeenCalledWith("open_dashboard_from_tray");
  });

  it("shows the backend-calculated cost when pricing data is available", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_overview") {
        return {
          total_events: 16,
          total_tokens_input: 4000000,
          total_tokens_output: 1669402,
          total_cost: 74.205283,
          cost_formatted: "$74.2053",
          cost_status: "estimated",
          provider_count: 1,
          high_confidence_count: 16,
          confidence_coverage: 1.0,
          latest_event_at: "2026-08-03T12:00:00Z",
          oldest_event_at: "2026-08-01T12:00:00Z",
        };
      }
      return null;
    });

    render(
      <I18nProvider>
        <TrayPopup />
      </I18nProvider>,
    );

    await waitFor(() => {
      expect(screen.getByText("$74.2053")).toBeInTheDocument();
      expect(screen.queryByText("Not available")).not.toBeInTheDocument();
    });
  });

  it("keeps the tray popup on one clean opaque surface without decorative frame stacking", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_overview") {
        return {
          total_events: 1,
          total_tokens_input: 1000,
          total_tokens_output: 500,
          provider_count: 1,
          high_confidence_count: 1,
          confidence_coverage: 1,
          latest_event_at: null,
          oldest_event_at: null,
        };
      }
      return null;
    });

    const { container } = render(
      <I18nProvider>
        <TrayPopup />
      </I18nProvider>,
    );

    await waitFor(() => expect(screen.getByText("lnwdeck")).toBeInTheDocument());

    expect(container.querySelector(".tray-window")).toHaveClass("tray-window-flat");
    expect(container.querySelector(".tray-window")).toHaveClass("tray-window-gradient");
    expect(container.querySelector(".tray-card")).toHaveClass("tray-card-flat");
    expect(container.querySelector(".tray-card")).toHaveClass("tray-card-modern");
    expect(container.querySelector(".tray-action-btn")).toHaveClass("tray-action-btn-filled");
    expect(container.querySelector(".tray-badge-ok")).toHaveClass("tray-badge-flat");
    expect(container.querySelector(".tray-badge-lnwdev")).toHaveClass("tray-badge-flat");
  });

  it("announces when the app is already up to date with a themed banner", async () => {
    const handlers = captureListeners();
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_overview") {
        return {
          total_events: 1,
          total_tokens_input: 1000,
          total_tokens_output: 500,
          provider_count: 1,
          high_confidence_count: 1,
          confidence_coverage: 1,
          latest_event_at: null,
          oldest_event_at: null,
        };
      }
      return null;
    });

    render(
      <I18nProvider>
        <TrayPopup />
      </I18nProvider>,
    );

    await waitFor(() => expect(handlers.has("update-up-to-date")).toBe(true));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();

    handlers.get("update-up-to-date")?.({
      payload: { version: "10.0.0" },
    });

    await waitFor(() =>
      expect(screen.getByRole("alert")).toBeInTheDocument(),
    );
    expect(screen.getByText(/latest version/i)).toBeInTheDocument();
    expect(screen.getByText(/10\.0\.0/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /dismiss/i })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /dismiss/i }));
    await waitFor(() =>
      expect(screen.queryByRole("alert")).not.toBeInTheDocument(),
    );
  });

  it("shows a themed error banner when the update check fails", async () => {
    const handlers = captureListeners();
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_overview") {
        return {
          total_events: 0,
          total_tokens_input: 0,
          total_tokens_output: 0,
          provider_count: 0,
          high_confidence_count: 0,
          confidence_coverage: 0,
          latest_event_at: null,
          oldest_event_at: null,
        };
      }
      return null;
    });

    render(
      <I18nProvider>
        <TrayPopup />
      </I18nProvider>,
    );

    await waitFor(() => expect(handlers.has("update-check-failed")).toBe(true));

    handlers.get("update-check-failed")?.({
      payload: { code: "NETWORK_ERROR" },
    });

    await waitFor(() =>
      expect(screen.getByRole("alert")).toBeInTheDocument(),
    );
    expect(screen.getByText(/NETWORK_ERROR/)).toBeInTheDocument();
  });
});
