import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { FloatingWidget, hasFetchedQuota, statusChip } from "./FloatingWidget";
import { translate } from "../../lib/i18n";
import * as native from "../../lib/native";

type RefreshHandler = (event: {
  payload: native.RefreshProgressEvent;
}) => void;
let refreshEventHandler: RefreshHandler | null = null;

vi.mock("../../lib/native", async (importOriginal) => {
  const actual = await importOriginal<typeof native>();
  return {
    ...actual,
    fetchQuotaDashboard: vi.fn(),
    fetchWidgetSettings: vi.fn(),
    setWidgetOpacity: vi.fn(),
    setWidgetLocked: vi.fn(),
    setWidgetProviders: vi.fn(),
    setWidgetView: vi.fn(),
    getWidgetPet: vi.fn(),
    fetchPetSpritesheetUrl: vi.fn(),
    hideWidgetWindow: vi.fn().mockResolvedValue(undefined),
    showMainWindow: vi.fn().mockResolvedValue(undefined),
    startRefresh: vi.fn().mockResolvedValue({
      started: true,
      already_running: false,
    }),
  };
});

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockImplementation(
    async (event: string, handler: RefreshHandler) => {
      if (event === "refresh-progress") {
        refreshEventHandler = handler;
      }
      return () => {};
    },
  ),
}));

/** A fixed reference point so reset countdowns are deterministic. */
const NOW = new Date(2026, 7, 4, 10, 0, 0).getTime();

function inHours(hours: number): string {
  return new Date(NOW + hours * 3_600_000).toISOString();
}

function windowWith(
  remainingPercent: number | null,
  overrides: Partial<native.QuotaWindowData> = {},
): native.QuotaWindowData {
  const limit = remainingPercent === null ? null : 1000;
  const remaining =
    remainingPercent === null
      ? null
      : Math.round((remainingPercent / 100) * 1000);
  return {
    window_key: "5h",
    label: "5-hour",
    scope: "rolling",
    kind: "tokens",
    used: remaining === null ? 1234 : 1000 - remaining,
    limit,
    remaining,
    used_percent: remainingPercent === null ? null : 100 - remainingPercent,
    remaining_percent: remainingPercent,
    reset_at: inHours(2.25),
    is_unlimited: false,
    confidence: "High",
    ...overrides,
  };
}

function provider(
  overrides: Partial<native.ProviderQuotaCard> = {},
): native.ProviderQuotaCard {
  return {
    provider_id: "anthropic_claude",
    display_name: "Claude",
    status: "fresh",
    plan: null,
    source: "local_estimate",
    collected_at: new Date(NOW - 30_000).toISOString(),
    stale_at: new Date(NOW + 3_600_000).toISOString(),
    error_code: null,
    windows: [windowWith(72)],
    ...overrides,
  };
}

function dashboard(
  providers: native.ProviderQuotaCard[],
): native.QuotaDashboardData {
  return {
    generated_at: new Date(NOW - 30_000).toISOString(),
    providers,
  };
}

describe("FloatingWidget", () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    vi.setSystemTime(NOW);
    vi.mocked(native.fetchWidgetSettings).mockResolvedValue({
      opacity: 1,
      locked: false,
      visible: true,
      selected_providers: [],
      view: "bars",
      pet_id: "",
      size_preset: "medium",
    });
    vi.mocked(native.fetchQuotaDashboard).mockReset();
    vi.mocked(native.setWidgetLocked).mockReset();
    vi.mocked(native.setWidgetProviders).mockReset();
    vi.mocked(native.setWidgetView).mockReset();
    vi.mocked(native.getWidgetPet).mockReset();
    vi.mocked(native.getWidgetPet).mockResolvedValue(null);
    vi.mocked(native.fetchPetSpritesheetUrl).mockReset();
    vi.mocked(native.fetchPetSpritesheetUrl).mockResolvedValue(
      "blob:mock-sprout",
    );
    vi.mocked(native.startRefresh).mockClear();
    refreshEventHandler = null;
    vi.mocked(native.hideWidgetWindow).mockClear();
    vi.mocked(native.showMainWindow).mockClear();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("uses compact icon-only header controls with localized hover explanations", async () => {
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(
      dashboard([provider()]),
    );
    const { container } = render(<FloatingWidget />);

    await waitFor(() => expect(screen.getByText("Claude")).toBeInTheDocument());

    const actionButtons = container.querySelectorAll(
      ".w-header-actions [data-widget-icon-action='true']",
    );
    expect(actionButtons).toHaveLength(6);
    expect(screen.getByRole("button", { name: "Refresh quota" })).toHaveAttribute(
      "title",
      "Refresh quota",
    );
    expect(screen.getByRole("button", { name: "Open" })).toHaveAttribute(
      "title",
      "Open the dashboard window",
    );
    expect(container.querySelector(".w-view-trigger")).toHaveAttribute(
      "data-widget-icon-action",
      "true",
    );
    expect(container.querySelector(".w-picker-trigger")).toHaveAttribute(
      "data-widget-icon-action",
      "true",
    );
  });

  it("shows a loading state before any data arrives", () => {
    vi.mocked(native.fetchQuotaDashboard).mockImplementation(
      () => new Promise(() => {}),
    );
    render(<FloatingWidget />);
    expect(screen.getByText("Loading")).toBeInTheDocument();
    expect(screen.queryByRole("progressbar")).not.toBeInTheDocument();
  });

  it("renders name, percentage, bar and reset time for a real limit", async () => {
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(
      dashboard([provider()]),
    );
    render(<FloatingWidget />);

    await waitFor(() => expect(screen.getByText("Claude")).toBeInTheDocument());
    expect(screen.getByText("72%")).toBeInTheDocument();
    expect(screen.getByText("Resets in 2h 15m")).toBeInTheDocument();
    expect(
      screen.getByText("Sliding 5-hour tokens window"),
    ).toBeInTheDocument();

    const bar = screen.getByRole("progressbar", {
      name: "Claude 5-hour remaining",
    });
    expect(bar).toHaveAttribute("aria-valuenow", "72");
    expect(bar).toHaveAttribute("aria-valuemin", "0");
    expect(bar).toHaveAttribute("aria-valuemax", "100");
    expect(bar).toHaveAttribute(
      "aria-valuetext",
      "72% remaining, Resets in 2h 15m",
    );
    expect(bar.firstElementChild).toHaveStyle({ width: "72%" });
  });

  it("colours the bar by severity", async () => {
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(
      dashboard([
        provider({
          provider_id: "a",
          display_name: "Normal",
          windows: [windowWith(72)],
        }),
        provider({
          provider_id: "b",
          display_name: "Warning",
          windows: [windowWith(41)],
        }),
        provider({
          provider_id: "c",
          display_name: "Critical",
          windows: [windowWith(8)],
        }),
      ]),
    );
    const { container } = render(<FloatingWidget />);

    await waitFor(() =>
      expect(screen.getByText("Critical")).toBeInTheDocument(),
    );
    expect(container.querySelectorAll(".w-bar-fill-normal")).toHaveLength(1);
    expect(container.querySelectorAll(".w-bar-fill-warning")).toHaveLength(1);
    expect(container.querySelectorAll(".w-bar-fill-critical")).toHaveLength(1);
    // The percentage text carries the same severity so colour is not the only cue.
    expect(container.querySelectorAll(".w-percent-critical")).toHaveLength(1);
  });

  it("says Unavailable and draws no bar when no limit is reported", async () => {
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(
      dashboard([provider({ windows: [windowWith(null)] })]),
    );
    render(<FloatingWidget />);

    await waitFor(() =>
      expect(screen.getByText("Unavailable")).toBeInTheDocument(),
    );
    expect(screen.queryByRole("progressbar")).not.toBeInTheDocument();
    expect(screen.getByText(/no limit reported/)).toBeInTheDocument();
  });

  it("states that the reset time is unknown when the provider omits it", async () => {
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(
      dashboard([provider({ windows: [windowWith(72, { reset_at: null })] })]),
    );
    render(<FloatingWidget />);

    await waitFor(() =>
      expect(screen.getByText("Reset time unavailable")).toBeInTheDocument(),
    );
  });

  it("renders a multi-day reset and a next-day reset in words", async () => {
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(
      dashboard([
        provider({
          provider_id: "codex",
          display_name: "Codex",
          windows: [
            windowWith(41, {
              window_key: "7d",
              label: "7-day",
              reset_at: inHours(104),
            }),
          ],
        }),
        provider({
          provider_id: "gemini",
          display_name: "Gemini Pro",
          windows: [
            windowWith(88, {
              window_key: "30d",
              label: "30-day",
              reset_at: inHours(26),
            }),
          ],
        }),
      ]),
    );
    render(<FloatingWidget />);

    await waitFor(() => expect(screen.getByText("OpenAI Codex")).toBeInTheDocument());
    expect(screen.queryByText("Codex")).not.toBeInTheDocument();
    expect(screen.getByText("Resets in 4d 8h")).toBeInTheDocument();
    expect(screen.getByText("Resets tomorrow")).toBeInTheDocument();
    expect(screen.getByText("41%")).toBeInTheDocument();
    expect(screen.getByText("88%")).toBeInTheDocument();
  });

  it("labels a stale reading as stale but still shows it", async () => {
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(
      dashboard([provider({ status: "stale" })]),
    );
    render(<FloatingWidget />);
    await waitFor(() => expect(screen.getByText("Stale")).toBeInTheDocument());
    expect(screen.getByText("72%")).toBeInTheDocument();
  });

  it("hides providers whose quota could not be fetched", async () => {
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(
      dashboard([
        provider({
          provider_id: "p1",
          display_name: "Rate",
          status: "rate_limited",
          error_code: "RATE_LIMITED",
          windows: [],
        }),
        provider({
          provider_id: "p2",
          display_name: "Auth",
          status: "auth_expired",
          error_code: "AUTH_EXPIRED",
          windows: [],
        }),
        provider({
          provider_id: "p3",
          display_name: "Unconfigured",
          status: "unavailable",
          error_code: "NOT_CONFIGURED",
          windows: [],
        }),
        provider({
          provider_id: "p4",
          display_name: "Broken",
          status: "error",
          error_code: "SOURCE_SCHEMA_MISMATCH",
          windows: [],
        }),
      ]),
    );
    render(<FloatingWidget />);

    await waitFor(() =>
      expect(screen.getByText("No quota data available")).toBeInTheDocument(),
    );
    expect(screen.queryByText("Rate")).not.toBeInTheDocument();
    expect(screen.queryByText("Auth")).not.toBeInTheDocument();
    expect(screen.queryByText("Unconfigured")).not.toBeInTheDocument();
    expect(screen.queryByText("Broken")).not.toBeInTheDocument();
    expect(screen.queryByText("SOURCE_SCHEMA_MISMATCH")).not.toBeInTheDocument();
    expect(screen.queryByRole("progressbar")).not.toBeInTheDocument();
    expect(screen.queryByText(/of \d+ providers/)).not.toBeInTheDocument();
  });

  it("shows only the providers that fetched quota data", async () => {
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(
      dashboard([
        provider({
          provider_id: "p1",
          display_name: "Claude",
          windows: [windowWith(72)],
        }),
        provider({
          provider_id: "p2",
          display_name: "Codex",
          status: "auth_expired",
          error_code: "AUTH_EXPIRED",
          windows: [],
        }),
        provider({
          provider_id: "p3",
          display_name: "Broken",
          status: "error",
          error_code: "SOURCE_SCHEMA_MISMATCH",
          windows: [],
        }),
      ]),
    );
    render(<FloatingWidget />);

    await waitFor(() => expect(screen.getByText("Claude")).toBeInTheDocument());
    expect(screen.queryByText("Codex")).not.toBeInTheDocument();
    expect(screen.queryByText("Broken")).not.toBeInTheDocument();
    expect(screen.getByText(/1 of 1 provider/)).toBeInTheDocument();
    const bars = screen.getAllByRole("progressbar");
    expect(bars).toHaveLength(1);
  });

  it("renders a local unlimited provider without a percentage", async () => {
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(
      dashboard([
        provider({
          provider_id: "ollama_local",
          display_name: "Ollama",
          windows: [
            windowWith(null, {
              window_key: "unlimited",
              label: "Unlimited",
              is_unlimited: true,
              reset_at: null,
            }),
          ],
        }),
      ]),
    );
    render(<FloatingWidget />);

    await waitFor(() =>
      expect(screen.getByText("Local runtime, no quota")).toBeInTheDocument(),
    );
    expect(screen.getByText("Unavailable")).toBeInTheDocument();
    expect(screen.queryByRole("progressbar")).not.toBeInTheDocument();
  });

  it("reports a backend failure instead of showing a quota", async () => {
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

  it("states that no provider has reported data yet", async () => {
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(dashboard([]));
    render(<FloatingWidget />);
    await waitFor(() =>
      expect(screen.getByText("No quota data yet")).toBeInTheDocument(),
    );
  });

  it("only shows the pinned providers and persists a change", async () => {
    vi.mocked(native.fetchWidgetSettings).mockResolvedValue({
      opacity: 1,
      locked: false,
      visible: true,
      selected_providers: ["anthropic_claude"],
      view: "bars",
      pet_id: "",
      size_preset: "medium",
    });
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(
      dashboard([
        provider(),
        provider({ provider_id: "openai_codex", display_name: "Codex" }),
      ]),
    );
    vi.mocked(native.setWidgetProviders).mockResolvedValue([
      "anthropic_claude",
      "openai_codex",
    ]);
    render(<FloatingWidget />);

    await waitFor(() => expect(screen.getByText("Claude")).toBeInTheDocument());
    const list = screen.getByRole("list", { name: "Provider quota" });
    expect(within(list).queryByText("Codex")).not.toBeInTheDocument();
    expect(screen.getByText(/1 of 2 provider/)).toBeInTheDocument();

    await userEvent.click(
      screen.getByRole("button", { name: "Choose providers" }),
    );
    expect(document.querySelector(".w-picker")?.parentElement).toBe(document.body);
    await userEvent.click(
      within(screen.getByRole("group", { name: "Providers shown" })).getByRole(
        "button",
        { name: "OpenAI Codex" },
      ),
    );

    expect(native.setWidgetProviders).toHaveBeenCalledWith([
      "anthropic_claude",
      "openai_codex",
    ]);
    await waitFor(() =>
      expect(screen.getByText(/2 of 2 provider/)).toBeInTheDocument(),
    );
  });

  it("keeps the provider filter outside the native drag region", async () => {
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(
      dashboard([
        provider(),
        provider({ provider_id: "openai_codex", display_name: "Codex" }),
      ]),
    );
    const { container } = render(<FloatingWidget />);

    await waitFor(() => expect(screen.getByText("Claude")).toBeInTheDocument());

    const header = container.querySelector(".w-header");
    expect(header).not.toHaveAttribute("data-tauri-drag-region");
    expect(container.querySelector(".w-brand")).toHaveAttribute(
      "data-tauri-drag-region",
    );

    const filter = screen.getByRole("button", { name: "Choose providers" });
    await userEvent.click(filter);
    expect(filter).toHaveAttribute("aria-expanded", "true");
    expect(
      within(screen.getByRole("group", { name: "Providers shown" })).getByRole(
        "button",
        { name: "OpenAI Codex" },
      ),
    ).toBeEnabled();
    expect(document.querySelector(".w-picker")).toHaveAttribute(
      "data-surface",
      "opaque",
    );
  });

  it("drives refresh, dashboard, lock and close through the backend", async () => {
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(
      dashboard([provider()]),
    );
    vi.mocked(native.setWidgetLocked).mockResolvedValue(true);
    render(<FloatingWidget />);

    await waitFor(() => expect(screen.getByText("Claude")).toBeInTheDocument());

    await userEvent.click(screen.getByRole("button", { name: "Refresh quota" }));
    expect(native.startRefresh).toHaveBeenCalledTimes(1);

    await userEvent.click(
      screen.getByRole("button", { name: "Open" }),
    );
    expect(native.showMainWindow).toHaveBeenCalledTimes(1);

    await userEvent.click(screen.getByRole("button", { name: "Lock widget" }));
    expect(native.setWidgetLocked).toHaveBeenCalledWith(true);
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Unlock widget" }),
      ).toHaveAttribute("aria-pressed", "true"),
    );

    await userEvent.click(screen.getByRole("button", { name: "Close widget" }));
    expect(native.hideWidgetWindow).toHaveBeenCalledTimes(1);
  });

  it("uses one opaque surface for the widget instead of a stacked glow frame", async () => {
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(
      dashboard([provider()]),
    );
    const { container } = render(<FloatingWidget />);

    await waitFor(() => expect(screen.getByText("Claude")).toBeInTheDocument());

    expect(container.querySelector(".w-root")).toHaveClass(
      "w-root-single-surface",
    );
  });

  it("stays disabled when refresh joins a job started by another surface", async () => {
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(
      dashboard([provider()]),
    );
    vi.mocked(native.startRefresh).mockResolvedValue({
      started: false,
      already_running: true,
    });
    render(<FloatingWidget />);

    await waitFor(() => expect(screen.getByText("Claude")).toBeInTheDocument());
    const refreshButton = screen.getByRole("button", { name: "Refresh quota" });
    await userEvent.click(refreshButton);

    await waitFor(() => expect(native.startRefresh).toHaveBeenCalledTimes(1));
    expect(refreshButton).toBeDisabled();
  });

  it("removes the drag region when locked and applies the stored opacity", async () => {
    vi.mocked(native.fetchWidgetSettings).mockResolvedValue({
      opacity: 0.8,
      locked: true,
      visible: true,
      selected_providers: [],
      view: "bars",
      pet_id: "",
      size_preset: "medium",
    });
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(
      dashboard([provider()]),
    );
    const { container } = render(<FloatingWidget />);

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Unlock widget" }),
      ).toBeInTheDocument(),
    );
    const root = container.querySelector(".w-root");
    expect(root).toHaveAttribute("data-locked", "true");
    expect(root).toHaveStyle({ opacity: "0.8" });
    expect(container.querySelector("[data-tauri-drag-region]")).toBeNull();
  });

  it("closes on Escape for keyboard users", async () => {
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(
      dashboard([provider()]),
    );
    render(<FloatingWidget />);
    await waitFor(() => expect(screen.getByText("Claude")).toBeInTheDocument());

    await userEvent.keyboard("{Escape}");
    expect(native.hideWidgetWindow).toHaveBeenCalledTimes(1);
  });

  it("keeps every control reachable by keyboard", async () => {
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(
      dashboard([provider()]),
    );
    render(<FloatingWidget />);
    await waitFor(() => expect(screen.getByText("Claude")).toBeInTheDocument());

    await userEvent.tab();
    expect(
      screen.getByRole("button", { name: "Refresh quota" }),
    ).toHaveFocus();

    await userEvent.tab();
    expect(
      screen.getByRole("button", { name: "Open" }),
    ).toHaveFocus();

    await userEvent.tab();
    expect(
      screen.getByRole("button", { name: "Widget layout" }),
    ).toHaveFocus();

    await userEvent.tab();
    expect(screen.getByRole("button", { name: "Lock widget" })).toHaveFocus();

    await userEvent.tab();
    expect(
      screen.getByRole("button", { name: "Choose providers" }),
    ).toHaveFocus();

    await userEvent.tab();
    expect(
      screen.getByRole("button", { name: "Close widget" }),
    ).toHaveFocus();
  });

  it("cleans up timers and event listeners on unmount", async () => {
    vi.mocked(native.fetchWidgetSettings).mockResolvedValue({
      opacity: 1,
      locked: false,
      visible: true,
      selected_providers: [],
      view: "pet",
      pet_id: "",
      size_preset: "medium",
    });
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(
      dashboard([provider()]),
    );
    const { container, unmount } = render(<FloatingWidget />);
    await waitFor(() => expect(screen.getByText("Claude")).toBeInTheDocument());
    expect(vi.getTimerCount()).toBeGreaterThan(0);

    await userEvent.click(screen.getByRole("button", { name: "Refresh quota" }));
    await act(async () => {
      refreshEventHandler?.({
        payload: {
          phase: "completed",
          completed: 1,
          total: 1,
          provider_id: null,
          error_code: null,
        },
      });
    });
    await waitFor(() =>
      expect(
        container.querySelector(".pet-react-celebrate"),
      ).not.toBeNull(),
    );

    unmount();
    expect(vi.getTimerCount()).toBe(0);
  });
});

describe("FloatingWidget pet view", () => {
  const petSettings = (
    overrides: Partial<native.WidgetSettingsData> = {},
  ): native.WidgetSettingsData => ({
    opacity: 1,
    locked: false,
    visible: true,
    selected_providers: [],
    view: "pet",
    pet_id: "",
      size_preset: "medium",
    ...overrides,
  });

  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    vi.setSystemTime(NOW);
    vi.mocked(native.fetchWidgetSettings).mockResolvedValue(petSettings());
  });

  it("shows every visible quota window in pet mode with full bar semantics", async () => {
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(
      dashboard([
        provider({
          windows: [
            windowWith(72, { window_key: "a" }),
            windowWith(null, { window_key: "b", label: "7-day" }),
          ],
        }),
        provider({
          provider_id: "openai_codex",
          display_name: "Codex",
          windows: [windowWith(41)],
        }),
      ]),
    );
    const { container } = render(<FloatingWidget />);

    await waitFor(() =>
      expect(screen.getByText("Quota mood: Worried")).toBeInTheDocument(),
    );

    // The mascot is decorative; the quota rows carry the information.
    expect(container.querySelector(".pet-svg")).toHaveAttribute(
      "aria-hidden",
      "true",
    );
    expect(screen.getByText("Claude")).toBeInTheDocument();
    expect(screen.getByText("OpenAI Codex")).toBeInTheDocument();
    expect(screen.getByText("72%")).toBeInTheDocument();
    expect(screen.getByText("41%")).toBeInTheDocument();
    // Every window carries its reset line; the exact countdown can shift by a
    // minute when the fake clock crosses the tick boundary during waitFor.
    expect(screen.getAllByText(/Resets (in|now|tomorrow)/)).toHaveLength(3);

    const bars = screen.getAllByRole("progressbar");
    expect(bars).toHaveLength(2);
    expect(bars[0]).toHaveAttribute("aria-valuenow", "72");
    expect(bars[1]).toHaveAttribute("aria-valuenow", "41");
  });

  it("renders Unavailable instead of a percentage for a null window in pet mode", async () => {
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(
      dashboard([provider({ windows: [windowWith(null)] })]),
    );
    const { container } = render(<FloatingWidget />);

    await waitFor(() =>
      expect(screen.getByText("Quota mood: Sleeping")).toBeInTheDocument(),
    );
    expect(screen.getByText("Unavailable")).toBeInTheDocument();
    expect(screen.getByText(/no limit reported/)).toBeInTheDocument();
    expect(container.querySelectorAll(".w-bar-unknown")).toHaveLength(1);
    expect(container.querySelector(".w-bar-fill")).toBeNull();
    expect(screen.queryByRole("progressbar")).not.toBeInTheDocument();
    expect(screen.queryByText("0%")).not.toBeInTheDocument();
    expect(screen.queryByText("100%")).not.toBeInTheDocument();
  });

  it("derives the pet mood only from the providers the widget shows", async () => {
    vi.mocked(native.fetchWidgetSettings).mockResolvedValue(
      petSettings({ selected_providers: ["shown"] }),
    );
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(
      dashboard([
        provider({
          provider_id: "shown",
          display_name: "Shown",
          windows: [windowWith(72)],
        }),
        provider({
          provider_id: "hidden",
          display_name: "Hidden",
          windows: [windowWith(3)],
        }),
      ]),
    );
    render(<FloatingWidget />);

    await waitFor(() =>
      expect(screen.getByText("Quota mood: Happy")).toBeInTheDocument(),
    );
    expect(screen.getByText("Shown")).toBeInTheDocument();
    expect(screen.queryByText("Hidden")).not.toBeInTheDocument();
    expect(screen.getByText(/1 of 2 provider/)).toBeInTheDocument();
  });

  it("keeps a failed provider from appearing or making the pet sad", async () => {
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(
      dashboard([
        provider({
          provider_id: "shown",
          display_name: "Shown",
          windows: [windowWith(72)],
        }),
        provider({
          provider_id: "failed",
          display_name: "Failed",
          status: "auth_expired",
          error_code: "AUTH_EXPIRED",
          windows: [],
        }),
      ]),
    );
    render(<FloatingWidget />);

    await waitFor(() =>
      expect(screen.getByText("Quota mood: Happy")).toBeInTheDocument(),
    );
    expect(screen.getByText("Shown")).toBeInTheDocument();
    expect(screen.queryByText("Failed")).not.toBeInTheDocument();
    expect(screen.getByText(/1 of 1 provider/)).toBeInTheDocument();
  });

  it("renders a locally imported pet instead of the built-in robot", async () => {
    vi.mocked(native.fetchWidgetSettings).mockResolvedValue(
      petSettings({ pet_id: "sprout" }),
    );
    vi.mocked(native.getWidgetPet).mockResolvedValue({
      id: "sprout",
      displayName: "Sprout",
      description: "A test pet",
      spritesheetPath: "spritesheet.webp",
      spriteVersionNumber: 2,
    });
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(
      dashboard([provider()]),
    );
    const { container } = render(<FloatingWidget />);

    await waitFor(() =>
      expect(screen.getByText("Quota mood: Happy")).toBeInTheDocument(),
    );
    await waitFor(() =>
      expect(container.querySelector(".pet-atlas[data-sprite-version]")).not.toBeNull(),
    );
    const atlas = container.querySelector(".pet-atlas");
    expect(atlas).toHaveAttribute("data-sprite-version", "2");
    expect(atlas).toHaveAttribute("aria-hidden", "true");
    expect((atlas as HTMLElement).style.backgroundImage).toContain(
      "blob:mock-sprout",
    );
    expect(container.querySelector(".pet-svg")).toBeNull();
  });

  it("falls back to the built-in robot when the selected pet is missing", async () => {
    vi.mocked(native.fetchWidgetSettings).mockResolvedValue(
      petSettings({ pet_id: "ghost" }),
    );
    vi.mocked(native.getWidgetPet).mockResolvedValue(null);
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(
      dashboard([provider()]),
    );
    const { container } = render(<FloatingWidget />);

    await waitFor(() =>
      expect(screen.getByText("Quota mood: Happy")).toBeInTheDocument(),
    );
    await waitFor(() =>
      expect(container.querySelector(".pet-svg")).not.toBeNull(),
    );
    expect(container.querySelector(".pet-atlas")).toBeNull();
  });

  it("keeps the pet stage out of the quota rows and draggable only when unlocked", async () => {
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(
      dashboard([provider()]),
    );
    const { container } = render(<FloatingWidget />);
    await waitFor(() => expect(screen.getByText("Claude")).toBeInTheDocument());

    const stage = container.querySelector(".pet-stage");
    expect(stage).not.toBeNull();
    expect(stage).toHaveAttribute("data-tauri-drag-region", "");
    expect(
      container.querySelector(".pet-stage button, .pet-stage a, .pet-stage select"),
    ).toBeNull();
  });

  it("stores the pet layout through the layout picker and renders it", async () => {
    vi.mocked(native.setWidgetView).mockResolvedValue("pet");
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(
      dashboard([provider()]),
    );
    render(<FloatingWidget />);
    await waitFor(() => expect(screen.getByText("Claude")).toBeInTheDocument());

    await userEvent.click(screen.getByRole("button", { name: "Widget layout" }));
    expect(document.querySelector(".w-view-options")?.parentElement).toBe(document.body);
    await userEvent.click(screen.getByRole("option", { name: "Pet" }));
    expect(native.setWidgetView).toHaveBeenCalledWith("pet");
    await waitFor(() =>
      expect(screen.getByText("Quota mood: Happy")).toBeInTheDocument(),
    );
  });

  it("keeps bars and rings reachable through the layout picker", async () => {
    vi.mocked(native.setWidgetView).mockImplementation(async (view) => view);
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(
      dashboard([provider()]),
    );
    const { container } = render(<FloatingWidget />);
    await waitFor(() => expect(screen.getByText("Claude")).toBeInTheDocument());

    await userEvent.click(screen.getByRole("button", { name: "Widget layout" }));
    expect(document.querySelector(".w-view-options")?.parentElement).toBe(document.body);
    await userEvent.click(screen.getByRole("option", { name: "Rings" }));
    await waitFor(() =>
      expect(container.querySelectorAll(".w-ring")).toHaveLength(1),
    );

    await userEvent.click(screen.getByRole("button", { name: "Widget layout" }));
    await userEvent.click(screen.getByRole("option", { name: "Bars" }));
    await waitFor(() =>
      expect(container.querySelectorAll(".w-bar")).toHaveLength(1),
    );
    expect(
      container.querySelector(".w-ring"),
    ).toBeNull();
  });

  it("falls back to bars for an invalid stored view", async () => {
    // The backend stores free-form strings, so an invalid layout is a real
    // possibility the widget must survive; the type cast documents that.
    vi.mocked(native.fetchWidgetSettings).mockResolvedValue({
      opacity: 1,
      locked: false,
      visible: true,
      selected_providers: [],
      view: "spirals",
    } as unknown as native.WidgetSettingsData);
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(
      dashboard([provider()]),
    );
    const { container } = render(<FloatingWidget />);
    await waitFor(() => expect(screen.getByText("Claude")).toBeInTheDocument());

    expect(container.querySelector(".w-root")).toHaveAttribute(
      "data-view",
      "bars",
    );
    expect(screen.queryByText(/Quota mood:/)).not.toBeInTheDocument();
  });

  it("celebrates briefly after a successful manual refresh and returns to the derived mood", async () => {
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(
      dashboard([provider()]),
    );
    const { container } = render(<FloatingWidget />);
    await waitFor(() =>
      expect(screen.getByText("Quota mood: Happy")).toBeInTheDocument(),
    );

    await userEvent.click(screen.getByRole("button", { name: "Refresh quota" }));
    await act(async () => {
      refreshEventHandler?.({
        payload: {
          phase: "completed",
          completed: 1,
          total: 1,
          provider_id: null,
          error_code: null,
        },
      });
    });
    await waitFor(() =>
      expect(container.querySelector(".pet-react-celebrate")).not.toBeNull(),
    );
    expect(container.querySelector(".pet-stage")).toHaveClass("pet-mood-happy");

    await act(async () => {
      await vi.advanceTimersByTimeAsync(2_600);
    });
    expect(container.querySelector(".pet-react-celebrate")).toBeNull();
    expect(container.querySelector(".pet-stage")).toHaveClass("pet-mood-happy");
  });

  it("does not celebrate when the manual refresh fails", async () => {
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue(
      dashboard([provider()]),
    );
    vi.mocked(native.startRefresh).mockRejectedValue(
      new Error("storage locked"),
    );
    const { container } = render(<FloatingWidget />);
    await waitFor(() =>
      expect(screen.getByText("Quota mood: Happy")).toBeInTheDocument(),
    );

    await userEvent.click(screen.getByRole("button", { name: "Refresh quota" }));
    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent("storage locked"),
    );
    expect(container.querySelector(".pet-react-celebrate")).toBeNull();
    expect(screen.getByText("Quota mood: Happy")).toBeInTheDocument();
  });
});

describe("statusChip", () => {
  // The default (English) translator maps keys to their English strings.
  const en = (key: string, vars?: Record<string, string>) =>
    translate("en", key, vars);

  it("names each provider state exactly once", () => {
    expect(statusChip("fresh", null, en).label).toBe("Live");
    expect(statusChip("stale", null, en).label).toBe("Stale");
    expect(statusChip("rate_limited", "RATE_LIMITED", en).label).toBe(
      "Rate limited",
    );
    expect(statusChip("auth_expired", "AUTH_EXPIRED", en).label).toBe(
      "Not authenticated",
    );
    expect(statusChip("unavailable", null, en).label).toBe("Unavailable");
    expect(statusChip("error", "BOOM", en).label).toBe("Error");
  });

  it("explains a provider that is only waiting for a key", () => {
    expect(statusChip("unavailable", "NOT_CONFIGURED", en).detail).toContain(
      "Add an API key",
    );
    expect(statusChip("unavailable", "SOURCE_UNAVAILABLE", en).detail).toContain(
      "No source",
    );
  });

  it("uses a non-alarming tone for a provider that is merely unconfigured", () => {
    expect(statusChip("unavailable", "NOT_CONFIGURED", en).tone).toBe("muted");
    expect(statusChip("auth_expired", "AUTH_EXPIRED").tone).toBe("error");
  });
});

describe("hasFetchedQuota", () => {
  it("keeps fresh and stale readings and hides failed collections", () => {
    expect(hasFetchedQuota("fresh")).toBe(true);
    expect(hasFetchedQuota("stale")).toBe(true);
    expect(hasFetchedQuota("unavailable")).toBe(false);
    expect(hasFetchedQuota("auth_expired")).toBe(false);
    expect(hasFetchedQuota("rate_limited")).toBe(false);
    expect(hasFetchedQuota("error")).toBe(false);
  });
});
