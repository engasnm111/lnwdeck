import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { FloatingWidget } from "./FloatingWidget";
import * as native from "../../lib/native";

vi.mock("../../lib/native", async (importOriginal) => {
  const actual = await importOriginal<typeof native>();
  return {
    ...actual,
    fetchQuotaDashboard: vi.fn(),
    fetchWidgetSettings: vi.fn(),
    setWidgetOpacity: vi.fn(),
    setWidgetLocked: vi.fn(),
    hideWidgetWindow: vi.fn().mockResolvedValue(undefined),
    showMainWindow: vi.fn().mockResolvedValue(undefined),
    refreshAll: vi.fn().mockResolvedValue({ usage: [], quota: [] }),
  };
});

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

const withLimit: native.QuotaWindowData = {
  window_key: "credits",
  label: "Credits",
  scope: "other",
  kind: "credits",
  used: 2_500_000,
  limit: 10_000_000,
  remaining: 7_500_000,
  used_percent: 25,
  remaining_percent: 75,
  reset_at: null,
  is_unlimited: false,
  confidence: "High",
};

const usageOnly: native.QuotaWindowData = {
  window_key: "5h",
  label: "5-hour",
  scope: "rolling",
  kind: "tokens",
  used: 1234,
  limit: null,
  remaining: null,
  used_percent: null,
  remaining_percent: null,
  reset_at: null,
  is_unlimited: false,
  confidence: "Medium",
};

const dashboard = (
  windows: native.QuotaWindowData[] = [withLimit],
  overrides: Partial<native.ProviderQuotaCard> = {},
): native.QuotaDashboardData => ({
  generated_at: new Date().toISOString(),
  providers: [
    {
      provider_id: "openrouter_api",
      display_name: "OpenRouter",
      status: "fresh",
      plan: "Paid",
      source: "provider_api",
      collected_at: new Date().toISOString(),
      stale_at: new Date(Date.now() + 3_600_000).toISOString(),
      error_code: null,
      windows,
      ...overrides,
    },
  ],
});

describe("FloatingWidget", () => {
  beforeEach(() => {
    vi.mocked(native.fetchWidgetSettings).mockResolvedValue({
      opacity: 1,
      locked: false,
      visible: true,
    });
    vi.mocked(native.fetchQuotaDashboard).mockReset();
    vi.mocked(native.setWidgetOpacity).mockReset();
    vi.mocked(native.setWidgetLocked).mockReset();
  });

  it("renders a remaining bar only when the provider reports a limit", async () => {
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(dashboard());
    render(<FloatingWidget />);

    await waitFor(() =>
      expect(screen.getByText("OpenRouter")).toBeInTheDocument(),
    );
    const bar = screen.getByRole("progressbar", { name: /Credits remaining/i });
    expect(bar).toHaveAttribute("aria-valuenow", "75");
    expect(screen.getByText(/75% left/)).toBeInTheDocument();
  });

  it("shows recorded usage as an estimate when no limit is reported", async () => {
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(
      dashboard([usageOnly]),
    );
    render(<FloatingWidget />);

    await waitFor(() => expect(screen.getByText(/estimate/)).toBeInTheDocument());
    expect(screen.queryByRole("progressbar")).not.toBeInTheDocument();
    expect(screen.queryByText(/% left/)).not.toBeInTheDocument();
  });

  it("renders an explicit error instead of fabricated data", async () => {
    vi.mocked(native.fetchQuotaDashboard).mockRejectedValue(
      new Error("quota dashboard: db locked"),
    );
    render(<FloatingWidget />);

    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent(
        "quota dashboard: db locked",
      ),
    );
    expect(screen.queryByRole("progressbar")).not.toBeInTheDocument();
  });

  it("states that there is no quota data yet", async () => {
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue({
      generated_at: new Date().toISOString(),
      providers: [],
    });
    render(<FloatingWidget />);

    await waitFor(() =>
      expect(screen.getByText("no quota data yet")).toBeInTheDocument(),
    );
  });

  it("shows the provider status and sanitized error code", async () => {
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(
      dashboard([], { status: "auth_expired", error_code: "AUTH_EXPIRED" }),
    );
    render(<FloatingWidget />);

    await waitFor(() =>
      expect(screen.getByText(/auth expired \(AUTH_EXPIRED\)/)).toBeInTheDocument(),
    );
    expect(screen.getByText("no quota data")).toBeInTheDocument();
  });

  it("persists opacity and lock through the backend", async () => {
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(dashboard());
    vi.mocked(native.setWidgetOpacity).mockResolvedValue(0.9);
    vi.mocked(native.setWidgetLocked).mockResolvedValue(true);
    render(<FloatingWidget />);

    await waitFor(() =>
      expect(screen.getByLabelText("Decrease opacity")).toBeInTheDocument(),
    );
    await userEvent.click(screen.getByLabelText("Decrease opacity"));
    expect(native.setWidgetOpacity).toHaveBeenCalledWith(0.9);

    await userEvent.click(screen.getByLabelText("Lock widget"));
    expect(native.setWidgetLocked).toHaveBeenCalledWith(true);
    await waitFor(() =>
      expect(screen.getByLabelText("Unlock widget")).toBeInTheDocument(),
    );
  });

  it("stops offering the drag region once locked", async () => {
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(dashboard());
    vi.mocked(native.fetchWidgetSettings).mockResolvedValue({
      opacity: 1,
      locked: true,
      visible: true,
    });
    const { container } = render(<FloatingWidget />);

    await waitFor(() =>
      expect(screen.getByLabelText("Unlock widget")).toBeInTheDocument(),
    );
    const root = container.querySelector(".widget-root");
    expect(root?.getAttribute("data-tauri-drag-region")).toBeNull();
  });
});
