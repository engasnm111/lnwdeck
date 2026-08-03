import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { CostsPage } from "./CostsPage";
import * as native from "../../lib/native";

vi.mock("../../lib/native", async (importOriginal) => {
  const actual = await importOriginal<typeof native>();
  return { ...actual, fetchCosts: vi.fn() };
});

const breakdown = (
  overrides: Partial<native.CostBreakdownData> = {},
): native.CostBreakdownData => ({
  window: "last_30d",
  generated_at: "2026-08-04T00:00:00Z",
  rows: [
    {
      provider_id: "anthropic_claude",
      model: "claude-test",
      request_count: 3,
      tokens_input: 2000,
      tokens_output: 500,
      cost: "0.003000",
      pricing_status: "priced",
    },
    {
      provider_id: "opencode",
      model: "glm-5",
      request_count: 2,
      tokens_input: 1000,
      tokens_output: 0,
      cost: null,
      pricing_status: "no catalog entry",
    },
  ],
  priced_total: "0.003000",
  priced_rows: 1,
  unpriced_rows: 1,
  unpriced_tokens: 1000,
  ...overrides,
});

describe("CostsPage", () => {
  beforeEach(() => {
    vi.mocked(native.fetchCosts).mockReset();
  });

  it("shows the priced total and marks unpriced models", async () => {
    vi.mocked(native.fetchCosts).mockResolvedValue(breakdown());
    render(<CostsPage />);

    await waitFor(() => expect(screen.getByText("claude-test")).toBeInTheDocument());
    expect(screen.getAllByText("0.003000").length).toBeGreaterThan(0);
    expect(screen.getByText("not priced")).toBeInTheDocument();
    expect(screen.getByText("no catalog entry")).toBeInTheDocument();
    expect(screen.getByText("Incomplete pricing")).toBeInTheDocument();
  });

  it("reports an empty window instead of a zero cost", async () => {
    vi.mocked(native.fetchCosts).mockResolvedValue(
      breakdown({ rows: [], priced_rows: 0, unpriced_rows: 0, unpriced_tokens: 0 }),
    );
    render(<CostsPage />);

    await waitFor(() =>
      expect(screen.getByText("No costs recorded")).toBeInTheDocument(),
    );
    expect(screen.queryByText("0.003000")).not.toBeInTheDocument();
  });

  it("surfaces a backend failure instead of rendering a table", async () => {
    vi.mocked(native.fetchCosts).mockRejectedValue(new Error("costs: db locked"));
    render(<CostsPage />);

    await waitFor(() =>
      expect(screen.getByText("costs: db locked")).toBeInTheDocument(),
    );
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });
});
