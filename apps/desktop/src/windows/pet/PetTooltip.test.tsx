import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { PetTooltip } from "./PetTooltip";
import { I18nProvider } from "../../app/I18nProvider";
import * as native from "../../lib/native";

vi.mock("../../lib/native", async (importOriginal) => {
  const actual = await importOriginal<typeof native>();
  return {
    ...actual,
    fetchQuotaDashboard: vi.fn(),
  };
});

describe("PetTooltip", () => {
  beforeEach(() => {
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue({
      generated_at: "2026-08-08T00:00:00Z",
      providers: [
        {
          provider_id: "openai_codex",
          display_name: "OpenAI Codex",
          connection_state: "connected",
          quota_support: "local estimate",
          status: "fresh",
          plan: null,
          source: "local_estimate",
          collected_at: "2026-08-08T00:00:00Z",
          stale_at: "2026-08-08T01:00:00Z",
          error_code: null,
          windows: [
            {
              window_key: "30d",
              label: "30-day",
              scope: "rolling",
              kind: "tokens",
              used: 18_400_000,
              limit: null,
              remaining: null,
              remaining_percent: null,
              used_percent: null,
              reset_at: null,
              is_unlimited: false,
              confidence: "High",
            },
          ],
        },
      ],
    });
  });

  it("shows the compact token value as themed text without a unit suffix", async () => {
    render(
      <I18nProvider>
        <PetTooltip visible />
      </I18nProvider>,
    );

    const token = await screen.findByRole("button", { name: /tokens:/i });
    expect(token).toHaveTextContent("18.4M");
    expect(token).not.toHaveTextContent(/tokens/i);

    expect(screen.getByText("OpenAI Codex")).toHaveClass(
      "pet-tooltip-bar-provider",
    );
    expect(screen.getByText("30-day")).toHaveClass("pet-tooltip-bar-window");
    expect(screen.getByTitle("OpenAI Codex — 30-day")).toBeInTheDocument();
  });

  it("keeps the provider and quota-window labels separately readable", async () => {
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue({
      generated_at: "2026-08-08T00:00:00Z",
      providers: [
        {
          provider_id: "opencode",
          display_name: "OpenCode (Go)",
          connection_state: "connected",
          quota_support: "supported",
          status: "fresh",
          plan: null,
          source: "provider_api",
          collected_at: "2026-08-08T00:00:00Z",
          stale_at: "2026-08-08T01:00:00Z",
          error_code: null,
          windows: [
            {
              window_key: "5h",
              label: "5-hour",
              scope: "rolling",
              kind: "tokens",
              used: 470,
              limit: 1000,
              remaining: 530,
              remaining_percent: 53,
              used_percent: 47,
              reset_at: "2026-08-08T05:00:00Z",
              is_unlimited: false,
              confidence: "High",
            },
          ],
        },
      ],
    });

    render(
      <I18nProvider>
        <PetTooltip visible />
      </I18nProvider>,
    );

    const provider = await screen.findByText("OpenCode (Go)");
    expect(provider).toHaveClass("pet-tooltip-bar-provider");
    expect(screen.getByText("5-hour")).toHaveClass("pet-tooltip-bar-window");
    expect(screen.getByTitle("OpenCode (Go) — 5-hour")).toBeInTheDocument();
  });
});
