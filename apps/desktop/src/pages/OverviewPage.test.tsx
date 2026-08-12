import { describe, expect, it, vi, beforeEach } from "vitest";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { listen } from "@tauri-apps/api/event";
import { OverviewPage } from "./OverviewPage";
import { I18nProvider } from "../app/I18nProvider";
import * as native from "../lib/native";

vi.mock("../lib/native", async (importOriginal) => {
  const actual = await importOriginal<typeof native>();
  return {
    ...actual,
    fetchUsageDashboard: vi.fn(),
    fetchQuotaDashboard: vi.fn(),
  };
});

const dashboard = (): native.UsageDashboardData => ({
  range: "month",
  generated_at: "2026-08-08T00:00:00Z",
  start: "2026-08-01T00:00:00Z",
  end: "2026-09-01T00:00:00Z",
  duration_days: 31,
  request_count: 3,
  tokens_input: 1_000_000,
  tokens_cached: 0,
  tokens_cache_write: 0,
  tokens_output: 234_567,
  tokens_reasoning: 0,
  total_tokens: 1_234_567,
  provider_count: 1,
  session_count: 1,
  providers: [
    {
      provider_id: "openai_codex",
      display_name: "OpenAI Codex",
      vendor: "OpenAI",
      request_count: 3,
      tokens_input: 1_000_000,
      tokens_cached: 0,
      tokens_cache_write: 0,
      tokens_output: 234_567,
      tokens_reasoning: 0,
      total_tokens: 1_234_567,
    },
  ],
  trend: [
    {
      bucket: "2026-08-01",
      request_count: 3,
      tokens_input: 1_000_000,
      tokens_cached: 0,
      tokens_cache_write: 0,
      tokens_output: 234_567,
      tokens_reasoning: 0,
      total_tokens: 1_234_567,
    },
  ],
  heatmap: Array.from({ length: 31 }, (_, index) => ({
    day: `2026-08-${String(index + 1).padStart(2, "0")}`,
    request_count: index === 0 ? 3 : 0,
    total_tokens: index === 0 ? 1_234_567 : 0,
  })),
  sessions: [
    {
      session_hash: "session-hash",
      display_name: "Session 01",
      request_count: 3,
      tokens_input: 1_000_000,
      tokens_cached: 0,
      tokens_cache_write: 0,
      tokens_output: 234_567,
      tokens_reasoning: 0,
      total_tokens: 1_234_567,
      first_seen_at: "2026-08-01T01:00:00Z",
      last_seen_at: "2026-08-01T02:30:00Z",
      providers: [
        {
          provider_id: "openai_codex",
          display_name: "OpenAI Codex",
          vendor: "OpenAI",
          request_count: 3,
          tokens_input: 1_000_000,
          tokens_cached: 0,
          tokens_cache_write: 0,
          tokens_output: 234_567,
          tokens_reasoning: 0,
          total_tokens: 1_234_567,
        },
      ],
    },
  ],
});

describe("OverviewPage", () => {
  beforeEach(() => {
    vi.mocked(native.fetchUsageDashboard).mockReset();
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue({
      generated_at: "2026-08-08T00:00:00Z",
      providers: [],
    });
    vi.mocked(listen).mockReset().mockResolvedValue(() => {});
  });

  it("keeps provider identity readable, puts daily breakdown before the other dashboard details, and renders a full calendar heatmap", async () => {
    const result = dashboard();
    result.trend = [
      result.trend[0],
      {
        ...result.trend[0],
        bucket: "2026-08-08",
        request_count: 2,
        total_tokens: 2_000_000,
      },
      {
        ...result.trend[0],
        bucket: "2099-12-31",
        request_count: 7,
        total_tokens: 9_000_000,
      },
    ];
    vi.mocked(native.fetchUsageDashboard).mockResolvedValue(result);

    const { container } = render(
      <I18nProvider>
        <OverviewPage />
      </I18nProvider>,
    );

    await waitFor(() => expect(screen.getAllByText("OpenAI Codex").length).toBeGreaterThan(0));

    expect(screen.queryByText("openai_codex")).not.toBeInTheDocument();
    expect(screen.getByRole("img", { name: "OpenAI Codex" })).toBeInTheDocument();
    expect(screen.queryByText("By device")).not.toBeInTheDocument();
    expect(container.querySelectorAll(".dashboard-heatmap-cell[data-day]")).toHaveLength(31);
    expect(container.querySelectorAll(".dashboard-heatmap-week")).toHaveLength(6);
    expect(container.querySelector('[data-provider-logo="openai"]')).not.toBeNull();

    const providerCard = container.querySelector(".dashboard-provider-breakdown");
    const dailyBreakdownCard = container.querySelector(".dashboard-daily-breakdown-card");
    expect(providerCard).not.toBeNull();
    expect(dailyBreakdownCard).not.toBeNull();
    expect(screen.getByText("Daily breakdown")).toBeInTheDocument();
    expect(screen.queryByText("Sessions")).not.toBeInTheDocument();
    expect(screen.getByText("2026-08-08")).toBeInTheDocument();
    expect(screen.getByText("2026-08-01")).toBeInTheDocument();
    expect(screen.queryByText("2099-12-31")).not.toBeInTheDocument();
    expect(
      providerCard!.compareDocumentPosition(dailyBreakdownCard!) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
  });

  it("does not derive a lowest reading from a provider whose collection failed", async () => {
    vi.mocked(native.fetchUsageDashboard).mockResolvedValue(dashboard());
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue({
      generated_at: "2026-08-08T00:00:00Z",
      providers: [
        {
          provider_id: "google_gemini",
          display_name: "Gemini",
          connection_state: "transient_error",
          quota_support: "supported",
          status: "unavailable",
          plan: null,
          source: "antigravity_ls",
          collected_at: "2026-08-08T00:00:00Z",
          stale_at: "2026-08-08T01:00:00Z",
          error_code: "SOURCE_REQUIRES_IDE",
          windows: [
            {
              window_key: "pro",
              label: "Gemini Pro",
              scope: "weekly",
              kind: "requests",
              used: 0,
              limit: null,
              remaining: null,
              remaining_percent: 100,
              used_percent: 0,
              reset_at: null,
              is_unlimited: false,
              confidence: "High",
            },
          ],
        },
      ],
    });

    render(
      <I18nProvider>
        <OverviewPage />
      </I18nProvider>,
    );

    await waitFor(() =>
      expect(
        screen.getByText(
          "No provider reports a real limit; quota is shown as usage estimates.",
        ),
      ).toBeInTheDocument(),
    );
    expect(screen.queryByText(/100% remaining/)).not.toBeInTheDocument();
  });

  it("keeps the daily breakdown bounded and exposes an accessible scroll viewport", async () => {    vi.mocked(native.fetchUsageDashboard).mockResolvedValue(dashboard());

    const { container } = render(
      <I18nProvider>
        <OverviewPage />
      </I18nProvider>,
    );

    await waitFor(() => expect(screen.getAllByText("OpenAI Codex").length).toBeGreaterThan(0));

    const dailyBreakdownCard = container.querySelector(".dashboard-daily-breakdown-card");
    const dailyViewport = container.querySelector(".dashboard-daily-breakdown-wrap");
    const dailyHead = container.querySelector(".dashboard-daily-breakdown-table thead");
    expect(dailyBreakdownCard).toHaveClass("dashboard-daily-breakdown-card");
    expect(dailyBreakdownCard).toHaveClass("dashboard-daily-breakdown-card-fixed");
    expect(dailyViewport).toHaveAttribute("role", "region");
    expect(dailyViewport).toHaveAttribute("tabindex", "0");
    expect(dailyViewport).toHaveAttribute("aria-label", "Daily token breakdown");
    expect(dailyViewport).toHaveClass("dashboard-daily-breakdown-wrap-fixed");
    expect(dailyHead).toHaveClass("ui-table-head-themed");
  });

  it("uses the selected calendar range for the daily breakdown rows", async () => {
    const month = dashboard();
    month.trend = [
      {
        ...month.trend[0],
        bucket: "2026-08-01",
        total_tokens: 100,
      },
      {
        ...month.trend[0],
        bucket: "2026-08-08",
        total_tokens: 803_200_000,
      },
    ];
    const day = dashboard();
    day.range = "day";
    day.trend = [{ ...month.trend[1] }];
    vi.mocked(native.fetchUsageDashboard)
      .mockResolvedValueOnce(month)
      .mockResolvedValue(day);
    const user = userEvent.setup();

    render(
      <I18nProvider>
        <OverviewPage />
      </I18nProvider>,
    );

    await waitFor(() => expect(screen.getByText("2026-08-08")).toBeInTheDocument());
    await user.click(screen.getByRole("tab", { name: "Day" }));
    await waitFor(() => expect(screen.getAllByText("803.2M").length).toBeGreaterThan(0));
    expect(screen.queryByText("2026-08-01")).not.toBeInTheDocument();
    expect(vi.mocked(native.fetchUsageDashboard).mock.calls.at(-1)?.[0]).toMatchObject({ range: "day" });
  });

  it("keeps the activity heatmap compact and height-stable", async () => {
    vi.mocked(native.fetchUsageDashboard).mockResolvedValue(dashboard());

    const { container } = render(
      <I18nProvider>
        <OverviewPage />
      </I18nProvider>,
    );

    await waitFor(() => expect(screen.getAllByText("OpenAI Codex").length).toBeGreaterThan(0));

    const heatmapCard = container.querySelector(".dashboard-heatmap-card");
    const heatmap = container.querySelector(".dashboard-heatmap");
    const weeks = container.querySelector(".dashboard-heatmap-weeks");
    expect(heatmapCard).toHaveClass("dashboard-heatmap-card");
    expect(heatmapCard).toHaveClass("dashboard-heatmap-card-fixed");
    expect(heatmap).toHaveClass("dashboard-heatmap");
    expect(weeks).toHaveStyle({ gridTemplateColumns: "repeat(6, 14px)" });
  });

  it("keeps usage trend date labels aligned without wrapping", async () => {
    const result = dashboard();
    result.trend = [
      { ...result.trend[0], bucket: "2026-07-10", total_tokens: 100 },
      { ...result.trend[0], bucket: "2026-07-11", total_tokens: 0 },
      { ...result.trend[0], bucket: "2026-07-12", total_tokens: 200 },
    ];
    vi.mocked(native.fetchUsageDashboard).mockResolvedValue(result);

    const { container } = render(
      <I18nProvider>
        <OverviewPage />
      </I18nProvider>,
    );

    await waitFor(() => expect(screen.getByText("07-11")).toBeInTheDocument());

    const labels = container.querySelectorAll(
      ".dashboard-trend .dashboard-trend-label",
    );
    expect(labels).toHaveLength(3);
    expect(labels[1]).toHaveAttribute("data-trend-label", "07-11");
  });

  it("keeps custom date fields inside the dark theme", async () => {
    vi.mocked(native.fetchUsageDashboard).mockResolvedValue(dashboard());
    const user = userEvent.setup();

    render(
      <I18nProvider>
        <OverviewPage />
      </I18nProvider>,
    );

    await waitFor(() => expect(screen.getAllByText("OpenAI Codex").length).toBeGreaterThan(0));
    await user.click(screen.getByRole("tab", { name: "Custom" }));

    expect(screen.getByLabelText("Start")).toHaveClass("dashboard-date-input");
    expect(screen.getByLabelText("End")).toHaveClass("dashboard-date-input");
  });

  it("provides visible themed controls for opening the day-month-year picker", async () => {
    vi.mocked(native.fetchUsageDashboard).mockResolvedValue(dashboard());
    const user = userEvent.setup();
    const { container } = render(
      <I18nProvider>
        <OverviewPage />
      </I18nProvider>,
    );

    await waitFor(() => expect(screen.getAllByText("OpenAI Codex").length).toBeGreaterThan(0));
    await user.click(screen.getByRole("tab", { name: "Custom" }));

    expect(container.querySelectorAll(".dashboard-date-control")).toHaveLength(2);
    expect(container.querySelectorAll(".dashboard-date-picker-trigger")).toHaveLength(2);
    expect(screen.getByRole("button", { name: "Start: Open date picker" })).toHaveAttribute(
      "data-date-picker-trigger",
      "true",
    );
    expect(screen.getByRole("button", { name: "End: Open date picker" })).toHaveAttribute(
      "data-date-picker-trigger",
      "true",
    );
  });

  it("does not let a slower previous range overwrite today's result", async () => {
    let resolveMonth: ((value: native.UsageDashboardData) => void) | undefined;
    const today = dashboard();
    today.range = "day";
    today.total_tokens = 9_000_000;
    today.tokens_input = 8_000_000;
    today.tokens_output = 1_000_000;
    const month = dashboard();

    vi.mocked(native.fetchUsageDashboard).mockImplementation((query) => {
      if (query.range === "month") {
        return new Promise((resolve) => {
          resolveMonth = resolve;
        });
      }
      return Promise.resolve(today);
    });
    const user = userEvent.setup();

    render(
      <I18nProvider>
        <OverviewPage />
      </I18nProvider>,
    );
    await user.click(screen.getByRole("tab", { name: "Day" }));
    await waitFor(() => expect(screen.getAllByText("9M").length).toBeGreaterThan(0));

    resolveMonth?.(month);
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(screen.getAllByText("9M").length).toBeGreaterThan(0);
  });

  it("debounces reloads while usage-updated events stream", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    let usageListener: (() => void) | undefined;
    vi.mocked(listen).mockImplementation(async (event, handler) => {
      if (event === "usage-updated") {
        usageListener = handler as unknown as () => void;
      }
      return () => {};
    });
    vi.mocked(native.fetchUsageDashboard).mockResolvedValue(dashboard());

    render(
      <I18nProvider>
        <OverviewPage />
      </I18nProvider>,
    );
    // Flush the initial mount load before counting reloads.
    await act(async () => {
      await Promise.resolve();
    });
    vi.mocked(native.fetchUsageDashboard).mockClear();

    usageListener?.();
    usageListener?.();
    usageListener?.();
    expect(native.fetchUsageDashboard).not.toHaveBeenCalled();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1_200);
    });
    expect(native.fetchUsageDashboard).toHaveBeenCalledTimes(1);
    vi.useRealTimers();
  });

  it("reloads the selected range when the background refresh finishes", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    let usageListener: (() => void) | undefined;
    vi.mocked(listen).mockImplementation(async (event, handler) => {
      if (event === "usage-updated") {
        usageListener = handler as unknown as () => void;
      }
      return () => {};
    });
    const empty = dashboard();
    empty.request_count = 0;
    empty.total_tokens = 0;
    empty.tokens_input = 0;
    empty.tokens_output = 0;
    empty.providers = [];
    empty.sessions = [];
    empty.heatmap = [];
    empty.trend = [];
    const refreshed = dashboard();
    refreshed.range = "day";
    refreshed.total_tokens = 803_200_000;
    refreshed.tokens_input = 16_800_000;
    refreshed.tokens_output = 1_700_000;

    vi.mocked(native.fetchUsageDashboard)
      .mockResolvedValueOnce(empty)
      .mockResolvedValue(refreshed);
    render(
      <I18nProvider>
        <OverviewPage />
      </I18nProvider>,
    );
    await screen.findByText("No usage in this range");

    usageListener?.();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1_200);
    });
    expect(screen.getAllByText("803.2M").length).toBeGreaterThan(0);
    vi.useRealTimers();
  });
});
