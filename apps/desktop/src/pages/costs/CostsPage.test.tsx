import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
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
      cost: "0.002500",
      pricing_status: "estimated",
    },
  ],
  priced_total: "0.003000",
  priced_rows: 1,
  estimated_rows: 1,
  unpriced_rows: 0,
  unpriced_tokens: 0,
  providers: ["anthropic_claude", "opencode"],
  ...overrides,
});

describe("CostsPage", () => {
  beforeEach(() => {
    vi.mocked(native.fetchCosts).mockReset();
  });

  it("shows the priced total and marks estimated models", async () => {
    vi.mocked(native.fetchCosts).mockResolvedValue(breakdown());
    render(<CostsPage />);

    await waitFor(() => expect(screen.getByText("claude-test")).toBeInTheDocument());
    expect(screen.getAllByText("0.003000").length).toBeGreaterThan(0);
    expect(screen.getAllByText("estimated").length).toBeGreaterThanOrEqual(2);
    expect(screen.getByText("0.002500")).toBeInTheDocument();
  });

  it("uses canonical provider/model labels and a translated unpriced status", async () => {
    vi.mocked(native.fetchCosts).mockResolvedValue(
      breakdown({
        rows: [
          {
            provider_id: "opencode",
            model: "OpenCode - local_sqlite",
            request_count: 1,
            tokens_input: 1_000,
            tokens_output: 0,
            cost: "",
            pricing_status: "no catalog entry",
          },
        ],
        priced_rows: 0,
        estimated_rows: 0,
      }),
    );
    render(<CostsPage />);

    expect((await screen.findAllByText("OpenCode (Go)")).length).toBeGreaterThanOrEqual(1);
    expect(screen.queryByText("opencode")).not.toBeInTheDocument();
    expect(screen.queryByText("OpenCode - local_sqlite")).not.toBeInTheDocument();
    expect(screen.getAllByText("not priced").length).toBeGreaterThanOrEqual(2);
  });

  it("reports an empty window instead of a zero cost", async () => {
    vi.mocked(native.fetchCosts).mockResolvedValue(
      breakdown({ rows: [], priced_rows: 0, estimated_rows: 0 }),
    );
    render(<CostsPage />);

    await waitFor(() =>
      expect(screen.getByText("No costs recorded")).toBeInTheDocument(),
    );
    expect(screen.queryByText("0.003000")).not.toBeInTheDocument();
  });

  it("filters by provider through the backend and keeps the dropdown full", async () => {
    vi.mocked(native.fetchCosts).mockResolvedValue(breakdown());
    render(<CostsPage />);

    const filter = await screen.findByLabelText("Provider");
    expect(screen.getByRole("option", { name: "All providers" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "OpenCode (Go)" })).toBeInTheDocument();

    await userEvent.selectOptions(filter, "opencode");
    await waitFor(() =>
      expect(native.fetchCosts).toHaveBeenCalledWith("last_30d", "opencode"),
    );
    expect(screen.getByRole("option", { name: "Claude" })).toBeInTheDocument();
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
