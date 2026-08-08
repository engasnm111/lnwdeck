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
  });
});
