import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { TrayPopup } from "./TrayPopup";
import { invoke } from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("TrayPopup Component", () => {
  beforeEach(() => {
    vi.clearAllMocks();
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

    render(<TrayPopup />);

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
      expect(screen.getByText("running")).toBeInTheDocument();
    });
  });

  it("invokes show_main_window when Open Dashboard button is clicked", async () => {
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
