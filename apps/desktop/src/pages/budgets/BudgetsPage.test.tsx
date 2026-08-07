import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { BudgetsPage } from "./BudgetsPage";
import * as native from "../../lib/native";

vi.mock("../../lib/native", async (importOriginal) => {
  const actual = await importOriginal<typeof native>();
  return {
    ...actual,
    fetchBudgets: vi.fn(),
    fetchProviders: vi.fn(),
    saveBudget: vi.fn(),
    deleteBudget: vi.fn(),
  };
});

const providers: native.DetailedProviderInfo[] = [
  {
    provider_id: "opencode",
    display_name: "OpenCode",
    vendor: "OpenCode",
    enabled: true,
    detected: true,
    source_type: "local_sqlite",
    usage_support: "local estimate",
    quota_support: "local estimate",
    auth_requirement: "local files",
    health_status: "Healthy",
    event_count: 4,
    total_tokens: 4000,
    last_sync: "2026-08-04T00:00:00Z",
    last_error_code: "",
    quota_summary: "used 4000 tokens (estimate)",
    reset_at: null,
    confidence: "Medium",
    cost_support: "Priced",
  },
];

const progress = (
  overrides: Partial<native.BudgetProgressData> = {},
): native.BudgetProgressData => ({
  budget: {
    id: 1,
    scope: { kind: "global" },
    period: "monthly",
    cost_limit: "10",
    token_limit: null,
    warn_percent: 80,
    enabled: true,
    created_at: "2026-08-01T00:00:00Z",
    updated_at: "2026-08-01T00:00:00Z",
  },
  period_start: "2026-07-05T00:00:00Z",
  request_count: 12,
  tokens_used: 9000,
  cost_used: "9.000000",
  unpriced_tokens: 0,
  cost_percent: 90,
  token_percent: null,
  state: "warning",
  ...overrides,
});

describe("BudgetsPage", () => {
  beforeEach(() => {
    vi.mocked(native.fetchProviders).mockResolvedValue(providers);
    vi.mocked(native.fetchBudgets).mockReset();
    vi.mocked(native.saveBudget).mockReset();
    vi.mocked(native.deleteBudget).mockReset();
  });

  it("states that no budget is configured rather than showing a healthy status", async () => {
    vi.mocked(native.fetchBudgets).mockResolvedValue({
      generated_at: "2026-08-04T00:00:00Z",
      budgets: [],
    });
    render(<BudgetsPage />);

    await waitFor(() =>
      expect(screen.getByText("No budgets configured")).toBeInTheDocument(),
    );
    expect(screen.queryByText("Under Limit")).not.toBeInTheDocument();
    expect(screen.queryByText("under")).not.toBeInTheDocument();
  });

  it("renders real progress against the configured limit", async () => {
    vi.mocked(native.fetchBudgets).mockResolvedValue({
      generated_at: "2026-08-04T00:00:00Z",
      budgets: [progress()],
    });
    render(<BudgetsPage />);

    await waitFor(() => expect(screen.getByText("Near limit")).toBeInTheDocument());
    expect(screen.getByText("9.000000 / 10")).toBeInTheDocument();
    const bar = screen.getByRole("progressbar", { name: "Cost budget used" });
    expect(bar).toHaveAttribute("aria-valuenow", "90");
    // No token limit was set, so the token bar must not claim a value.
    expect(
      screen.getByRole("img", { name: /Token budget used: no limit reported/i }),
    ).toBeInTheDocument();
  });

  it("reports a rejected budget instead of pretending it saved", async () => {
    vi.mocked(native.fetchBudgets).mockResolvedValue({
      generated_at: "2026-08-04T00:00:00Z",
      budgets: [],
    });
    vi.mocked(native.saveBudget).mockRejectedValue(
      new Error("a budget needs a cost limit or a token limit"),
    );
    render(<BudgetsPage />);

    await waitFor(() =>
      expect(screen.getByText("No budgets configured")).toBeInTheDocument(),
    );
    await userEvent.click(screen.getByRole("button", { name: "Save budget" }));

    await waitFor(() =>
      expect(
        screen.getByText("a budget needs a cost limit or a token limit"),
      ).toBeInTheDocument(),
    );
  });

  it("saves a budget with the entered values", async () => {
    vi.mocked(native.fetchBudgets).mockResolvedValue({
      generated_at: "2026-08-04T00:00:00Z",
      budgets: [],
    });
    vi.mocked(native.saveBudget).mockResolvedValue(7);
    render(<BudgetsPage />);

    await waitFor(() =>
      expect(screen.getByLabelText("Cost limit")).toBeInTheDocument(),
    );
    await userEvent.type(screen.getByLabelText("Cost limit"), "25.50");
    await userEvent.click(screen.getByRole("button", { name: "Save budget" }));

    await waitFor(() => expect(native.saveBudget).toHaveBeenCalledTimes(1));
    expect(native.saveBudget).toHaveBeenCalledWith(
      expect.objectContaining({
        scope: "global",
        period: "monthly",
        cost_limit: "25.50",
        warn_percent: 80,
        enabled: true,
      }),
    );
  });

  it("keeps budget fields and actions in separate aligned rows", async () => {
    vi.mocked(native.fetchBudgets).mockResolvedValue({
      generated_at: "2026-08-04T00:00:00Z",
      budgets: [],
    });
    render(<BudgetsPage />);

    await waitFor(() =>
      expect(screen.getByLabelText("Cost limit")).toBeInTheDocument(),
    );

    const fields = document.querySelector(".budget-form-fields");
    const actions = document.querySelector(".budget-form-actions");
    expect(fields).not.toBeNull();
    expect(actions).not.toBeNull();
    expect(fields).toContainElement(screen.getByLabelText("Cost limit"));
    expect(fields).toContainElement(screen.getByLabelText("Warn at"));
    expect(actions).toContainElement(
      screen.getByRole("switch", { name: "Enabled" }),
    );
    expect(actions).toContainElement(
      screen.getByRole("button", { name: "Save budget" }),
    );
  });
});
