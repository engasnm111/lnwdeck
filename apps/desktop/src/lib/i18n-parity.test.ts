import fs from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";
import {
  LANGUAGES,
  translationKeys,
  translationPlaceholders,
  type LanguageCode,
} from "./i18n";

describe("localization parity", () => {
  it("keeps translation keys and placeholder names identical in every locale", () => {
    const reference = translationKeys("en");
    const placeholders = translationPlaceholders("en");
    for (const { code } of LANGUAGES) {
      expect(translationKeys(code), code).toEqual(reference);
      expect(translationPlaceholders(code), code).toEqual(placeholders);
    }
  });

  it("rejects user-facing refresh and tray copy embedded in shipped components", () => {
    const files = [
      "../app/AppShell.tsx",
      "../pages/OverviewPage.tsx",
      "../pages/pet/PetPage.tsx",
      "../pages/settings/SettingsPage.tsx",
      "../windows/widget/FloatingWidget.tsx",
      "../windows/pet/DesktopPet.tsx",
      "../windows/pet/PetTooltip.tsx",
      "../windows/tray/TrayPopup.tsx",
      "../../../../packages/ui/src/ErrorState.tsx",
      "../../../../packages/ui/src/EmptyState.tsx",
      "../../../../packages/ui/src/LoadingState.tsx",
    ].map((relative) => path.resolve(__dirname, relative));
    const forbidden = [
      "Sync now",
      "Open Dashboard",
      "Total Tokens",
      "Loading...",
      ">running<",
      "No usage yet",
      "Pet options",
      "<Card title=\"Privacy\">",
      "API key`}",
      "Could not load this view",
      "Try again",
      "No data yet",
      "Nothing has been recorded for this view.",
    ];
    const offenders = files.flatMap((file) => {
      const source = fs.readFileSync(file, "utf8");
      return forbidden
        .filter((text) => source.includes(text))
        .map((text) => `${path.basename(file)}:${text}`);
    });
    expect(offenders).toEqual([]);
  });

  it("declares all supported locale codes in the typed dictionary", () => {
    const codes: LanguageCode[] = LANGUAGES.map((language) => language.code);
    expect(codes).toEqual(["en", "th", "zh", "ja", "ko", "de", "fr", "es", "ru"]);
  });
});
