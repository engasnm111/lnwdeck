import { describe, expect, it } from "vitest";
import { pickPetQuip, deriveQuipData } from "./petQuips";
import { LANGUAGES, type LanguageCode } from "../../lib/i18n";
import type { ProviderQuotaCard } from "../../lib/native";

function card(
  overrides: Partial<ProviderQuotaCard> = {},
): ProviderQuotaCard {
  return {
    provider_id: "google_gemini",
    display_name: "Gemini",
    connection_state: "connected",
    quota_support: "supported",
    status: "fresh",
    plan: null,
    source: "antigravity_ls",
    collected_at: "2026-08-08T00:00:00Z",
    stale_at: "2026-08-08T01:00:00Z",
    error_code: null,
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
    ...overrides,
  };
}

describe("deriveQuipData", () => {
  it("ignores windows from a provider whose collection failed", () => {
    const data = deriveQuipData({
      generated_at: "2026-08-08T00:00:00Z",
      providers: [
        card({
          connection_state: "transient_error",
          status: "unavailable",
          error_code: "SOURCE_REQUIRES_IDE",
        }),
      ],
    });
    expect(data.lowestRemainingPercent).toBeNull();
    expect(data.todayTokens).toBe(0);
  });

  it("uses usable readings from connected providers", () => {
    const data = deriveQuipData({
      generated_at: "2026-08-08T00:00:00Z",
      providers: [
        card({
          status: "stale",
          windows: [
            {
              window_key: "pro",
              label: "Gemini Pro",
              scope: "weekly",
              kind: "requests",
              used: 40,
              limit: 100,
              remaining: 60,
              remaining_percent: 60,
              used_percent: 40,
              reset_at: null,
              is_unlimited: false,
              confidence: "High",
            },
          ],
        }),
      ],
    });
    expect(data.lowestRemainingPercent).toBe(60);
    expect(data.todayTokens).toBe(40);
  });
});


describe("pickPetQuip", () => {
  it("reports real token usage when data exists", () => {
    const original = Math.random;
    Math.random = () => 0;
    try {
      const quip = pickPetQuip({
        todayTokens: 12_500_000,
        costUsd: 4.2,
        currencySymbol: "$",
        lowestRemainingPercent: 23,
        plan: "Pro",
      });
      expect(quip).toBe("Used 12.5M tokens today");
    } finally {
      Math.random = original;
    }
  });

  it("always returns a personality line when there is no quota data", () => {
    const quip = pickPetQuip({
      todayTokens: 0,
      costUsd: 0,
      currencySymbol: "$",
      lowestRemainingPercent: null,
      plan: null,
    });
    expect(quip.length).toBeGreaterThan(0);
  });

  it("formats compact numbers", () => {
    const original = Math.random;
    Math.random = () => 0;
    try {
      const quip = pickPetQuip({
        todayTokens: 1_234_567_890,
        costUsd: 0,
        currencySymbol: "$",
        lowestRemainingPercent: null,
        plan: null,
      });
      expect(quip).toBe("Used 1.2B tokens today");
    } finally {
      Math.random = original;
    }
  });

  it("uses the shared compact formatter without unnecessary zero decimals", () => {
    const original = Math.random;
    Math.random = () => 0;
    try {
      const quip = pickPetQuip({
        todayTokens: 1_000_000,
        costUsd: 0,
        currencySymbol: "$",
        lowestRemainingPercent: null,
        plan: null,
      });
      expect(quip).toBe("Used 1M tokens today");
    } finally {
      Math.random = original;
    }
  });

  it("provides a localized click speech line for every supported language", () => {
    const original = Math.random;
    Math.random = () => 0;
    const markers: Record<LanguageCode, string> = {
      en: "Used",
      th: "วันนี้ใช้ไป",
      zh: "今天已使用",
      ja: "今日",
      ko: "오늘",
      de: "Heute",
      fr: "jetons utilisés",
      es: "tokens usados",
      ru: "Сегодня использовано",
    };

    try {
      for (const { code } of LANGUAGES) {
        const quip = pickPetQuip(
          {
            todayTokens: 12_500_000,
            costUsd: 0,
            currencySymbol: "$",
            lowestRemainingPercent: null,
            plan: null,
          },
          code,
        );
        expect(quip, `missing localized pet speech for ${code}`).toContain(
          markers[code],
        );
      }
    } finally {
      Math.random = original;
    }
  });
});
