import { describe, expect, it } from "vitest";
import { pickPetQuip } from "./petQuips";
import { LANGUAGES, type LanguageCode } from "../../lib/i18n";

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
