import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { ModelsPage } from "./ModelsPage";
import * as native from "../../lib/native";

vi.mock("../../lib/native", async (importOriginal) => {
  const actual = await importOriginal<typeof native>();
  return { ...actual, fetchUsageHistory: vi.fn() };
});

const history = (
  overrides: Partial<native.UsageHistoryData> = {},
): native.UsageHistoryData => ({
  window: "last_7d",
  generated_at: "2026-08-04T00:00:00Z",
  since: "2026-07-28T00:00:00Z",
  request_count: 4,
  tokens_input: 3000,
  tokens_output: 1000,
  models: [
    {
      model: "glm-5",
      provider_id: "opencode",
      request_count: 4,
      tokens_input: 3000,
      tokens_output: 1000,
      token_share_percent: 100,
      first_seen_at: "2026-08-01T00:00:00Z",
      last_seen_at: "2026-08-04T00:00:00Z",
    },
  ],
  daily: [
    { day: "2026-08-04", request_count: 4, tokens_input: 3000, tokens_output: 1000 },
  ],
  providers: ["opencode", "anthropic_claude"],
  ...overrides,
});

describe("ModelsPage", () => {
  beforeEach(() => {
    vi.mocked(native.fetchUsageHistory).mockReset();
  });

  it("renders recorded usage per model", async () => {
    vi.mocked(native.fetchUsageHistory).mockResolvedValue(history());
    render(<ModelsPage />);

    await waitFor(() => expect(screen.getByText("glm-5")).toBeInTheDocument());
    expect(screen.getByText("100.0%")).toBeInTheDocument();
    expect(
      screen.getByRole("progressbar", { name: /glm-5 share of tokens/i }),
    ).toBeInTheDocument();
  });

  it("shows no share when the window recorded no tokens", async () => {
    vi.mocked(native.fetchUsageHistory).mockResolvedValue(
      history({
        models: [
          {
            model: "quiet-model",
            provider_id: "opencode",
            request_count: 0,
            tokens_input: 0,
            tokens_output: 0,
            token_share_percent: null,
            first_seen_at: null,
            last_seen_at: null,
          },
        ],
      }),
    );
    render(<ModelsPage />);

    await waitFor(() =>
      expect(screen.getByText("no tokens recorded")).toBeInTheDocument(),
    );
    expect(
      screen.getByRole("img", { name: /no limit reported/i }),
    ).toBeInTheDocument();
  });

  it("states that nothing was recorded instead of showing zeros", async () => {
    vi.mocked(native.fetchUsageHistory).mockResolvedValue(
      history({ models: [], daily: [], request_count: 0 }),
    );
    render(<ModelsPage />);

    await waitFor(() =>
      expect(screen.getByText("No model usage recorded")).toBeInTheDocument(),
    );
  });

  it("surfaces a backend failure", async () => {
    vi.mocked(native.fetchUsageHistory).mockRejectedValue(
      new Error("usage history: db locked"),
    );
    render(<ModelsPage />);

    await waitFor(() =>
      expect(screen.getByText("usage history: db locked")).toBeInTheDocument(),
    );
  });
});
