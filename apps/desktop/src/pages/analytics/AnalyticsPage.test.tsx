import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { AnalyticsPage } from "./AnalyticsPage";
import * as native from "../../lib/native";

vi.mock("../../lib/native", async (importOriginal) => {
  const actual = await importOriginal<typeof native>();
  return { ...actual, fetchAnalytics: vi.fn() };
});

describe("AnalyticsPage", () => {
  beforeEach(() => {
    vi.mocked(native.fetchAnalytics).mockReset().mockResolvedValue({
      rows: [],
      available_providers: ["opencode", "anthropic_claude"],
      available_models: ["glm-5", "claude-test"],
    });
  });

  it("renders analytics heading", () => {
    render(<AnalyticsPage />);
    expect(screen.getByRole("heading", { name: "Analytics" })).toBeVisible();
  });

  it("renders filter controls with accessible labels", () => {
    render(<AnalyticsPage />);
    expect(screen.getByLabelText("Provider")).toBeInTheDocument();
    expect(screen.getByLabelText("Model")).toBeInTheDocument();
    expect(screen.getByLabelText("Confidence")).toBeInTheDocument();
  });

  it("shows empty state when no data", async () => {
    render(<AnalyticsPage />);
    expect(await screen.findByText(/no usage data yet/i)).toBeInTheDocument();
  });

  it("all pages are keyboard reachable", () => {
    render(<AnalyticsPage />);
    expect(screen.getByLabelText("Provider")).not.toBeDisabled();
    expect(screen.getByLabelText("Model")).not.toBeDisabled();
    expect(screen.getByLabelText("Confidence")).not.toBeDisabled();
  });

  it("renders canonical provider/model labels and does not turn an unpriced event into zero cost", async () => {
    vi.mocked(native.fetchAnalytics).mockResolvedValue({
      rows: [
        {
          id: "event-1",
          timestamp: "2026-08-08T00:00:00Z",
          provider_id: "openai_codex",
          model: "unknown",
          tokens_input: 1_000,
          tokens_cached: 0,
          tokens_cache_write: 0,
          tokens_output: 234,
          tokens_reasoning: 0,
          confidence: "Low",
          cost: "",
          pricing_status: "unpriced",
        },
      ],
      available_providers: ["openai_codex"],
      available_models: ["unknown"],
    });

    render(<AnalyticsPage />);

    expect((await screen.findAllByText("OpenAI Codex")).length).toBeGreaterThanOrEqual(2);
    expect(screen.getAllByText("Unknown model").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Low").length).toBeGreaterThanOrEqual(2);
    expect(screen.queryByText("openai_codex")).not.toBeInTheDocument();
    expect(screen.queryByText("$0.0000")).not.toBeInTheDocument();
  });

  it("aligns the time column to the left, not like numeric columns", async () => {
    vi.mocked(native.fetchAnalytics).mockResolvedValue({
      rows: [
        {
          id: "event-1",
          timestamp: "2026-08-08T00:00:00Z",
          provider_id: "opencode",
          model: "glm-5",
          tokens_input: 100,
          tokens_cached: 0,
          tokens_cache_write: 0,
          tokens_output: 50,
          tokens_reasoning: 0,
          confidence: "High",
          cost: "0.001000",
          pricing_status: "priced",
        },
      ],
      available_providers: ["opencode"],
      available_models: ["glm-5"],
    });

    const { container } = render(<AnalyticsPage />);
    const timeCell = await screen.findByText(/2026-08-08/);
    const td = timeCell.closest("td");
    expect(td).not.toBeNull();
    expect(td!.classList.contains("ui-table-numeric")).toBe(false);
    expect(container.querySelectorAll("td.ui-table-numeric")).toHaveLength(0);
  });
});
