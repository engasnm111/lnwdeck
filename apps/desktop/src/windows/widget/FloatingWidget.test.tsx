import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { FloatingWidget } from "./FloatingWidget";
import * as native from "../../lib/native";
import type { QuotaDashboardData } from "../../lib/native";

vi.mock("../../lib/native", async (importOriginal) => {
  const actual = await importOriginal<typeof native>();
  return {
    ...actual,
    fetchQuotaDashboard: vi.fn(),
    hideWidgetWindow: vi.fn().mockResolvedValue(undefined),
    showMainWindow: vi.fn().mockResolvedValue(undefined),
  };
});

vi.mock("@tauri-apps/api/event", () => ({
  listen: () => Promise.resolve(() => undefined),
}));

function fixture(): QuotaDashboardData {
  return {
    generated_at: "2026-08-04T12:00:00Z",
    providers: [
      {
        provider_id: "anthropic_claude",
        display_name: "Claude",
        status: "fresh",
        plan: "Max",
        source: "cli_api",
        collected_at: "2026-08-04T11:55:00Z",
        stale_at: "2026-08-04T12:55:00Z",
        error_code: null,
        windows: [
          {
            window_key: "5h",
            label: "5-hour",
            scope: "rolling",
            kind: "requests",
            used: 40,
            limit: 100,
            remaining: 60,
            used_percent: 40,
            remaining_percent: 60,
            reset_at: new Date(Date.now() + 134 * 60_000).toISOString(),
            is_unlimited: false,
            confidence: "High",
          },
        ],
      },
      {
        provider_id: "opencode",
        display_name: "OpenCode",
        status: "fresh",
        plan: null,
        source: "local_estimate",
        collected_at: "2026-08-04T11:55:00Z",
        stale_at: "2026-08-04T12:55:00Z",
        error_code: null,
        windows: [
          {
            window_key: "5h",
            label: "5-hour",
            scope: "rolling",
            kind: "tokens",
            used: 775,
            limit: 0,
            remaining: 0,
            used_percent: 0,
            remaining_percent: 100,
            reset_at: null,
            is_unlimited: false,
            confidence: "Medium",
          },
        ],
      },
      {
        provider_id: "ollama_local",
        display_name: "Ollama",
        status: "fresh",
        plan: null,
        source: "local_api",
        collected_at: "2026-08-04T11:55:00Z",
        stale_at: "2026-08-04T12:55:00Z",
        error_code: null,
        windows: [
          {
            window_key: "unlimited",
            label: "Unlimited",
            scope: "other",
            kind: "requests",
            used: 0,
            limit: 0,
            remaining: 0,
            used_percent: 0,
            remaining_percent: 100,
            reset_at: null,
            is_unlimited: true,
            confidence: "High",
          },
        ],
      },
    ],
  };
}

describe("FloatingWidget", () => {
  beforeEach(() => {
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(fixture());
    vi.clearAllMocks();
  });

  it("renders a remaining-quota bar per provider", async () => {
    render(<FloatingWidget />);

    expect(await screen.findByText("Claude")).toBeInTheDocument();
    expect(await screen.findByText(/60% left/)).toBeInTheDocument();
    expect(screen.getByText(/resets \d+h \d+m/)).toBeInTheDocument();
    expect(screen.getByText("OpenCode")).toBeInTheDocument();
    expect(screen.getByText(/used 775 tokens/)).toBeInTheDocument();
    expect(screen.getByText(/estimate/)).toBeInTheDocument();
    expect(screen.getByText("Ollama")).toBeInTheDocument();
    expect(screen.getByText("Local / Unlimited")).toBeInTheDocument();
  });

  it("renders explicit error state instead of fabricated data", async () => {
    vi.mocked(native.fetchQuotaDashboard).mockRejectedValue(
      new Error("command failed"),
    );
    render(<FloatingWidget />);

    expect(await screen.findByText("quota unavailable")).toBeInTheDocument();
    expect(screen.queryByText("Claude")).not.toBeInTheDocument();
  });

  it("renders an empty state when no provider has quota", async () => {
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue({
      generated_at: "2026-08-04T12:00:00Z",
      providers: [],
    });
    render(<FloatingWidget />);

    expect(await screen.findByText("no quota data yet")).toBeInTheDocument();
  });

  it("shows stale and error badges truthfully", async () => {
    const data = fixture();
    data.providers[0].status = "stale";
    data.providers[1].status = "auth_expired";
    data.providers[1].error_code = "AUTH_EXPIRED";
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(data);
    render(<FloatingWidget />);

    expect(await screen.findByText("stale")).toBeInTheDocument();
    expect(screen.getByText(/auth expired \(AUTH_EXPIRED\)/)).toBeInTheDocument();
  });

  it("refresh button reloads the dashboard", async () => {
    const user = userEvent.setup();
    render(<FloatingWidget />);
    await screen.findByText("Claude");

    await user.click(screen.getByRole("button", { name: /refresh quota/i }));

    await waitFor(() => {
      expect(native.fetchQuotaDashboard).toHaveBeenCalledTimes(2);
    });
  });

  it("lock toggle switches drag region and persists state", async () => {
    const user = userEvent.setup();
    render(<FloatingWidget />);
    await screen.findByText("Claude");

    const root = document.querySelector(".widget-root");
    expect(root?.getAttribute("data-tauri-drag-region")).toBe("");

    await user.click(screen.getByRole("button", { name: /lock widget/i }));
    expect(root?.getAttribute("data-tauri-drag-region")).toBeNull();
    expect(JSON.parse(localStorage.getItem("lnwdeck_widget_state")!)).toMatchObject({
      lockMode: "locked",
    });
  });
});
