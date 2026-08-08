import { translationKeys, translationPlaceholders, LANGUAGES } from "./i18n";
import { describe, it, expect } from "vitest";

describe("locale parity", () => {
  it("every locale has identical key sets", () => {
    const baseline = translationKeys("en");
    for (const lang of LANGUAGES) {
      const keys = translationKeys(lang.code);
      expect(keys).toEqual(baseline);
    }
  });

  it("every locale uses identical placeholders per key", () => {
    const baseline = translationPlaceholders("en");
    for (const lang of LANGUAGES) {
      const placeholders = translationPlaceholders(lang.code);
      for (const [key, names] of Object.entries(baseline)) {
        expect(placeholders[key], `${lang}:${key}`).toEqual(names);
      }
    }
  });
});
