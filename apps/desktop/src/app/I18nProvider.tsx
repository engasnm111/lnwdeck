import { useEffect, useMemo, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { fetchSettings } from "../lib/native";
import {
  I18nContext,
  translate,
  LANGUAGES,
  type I18n,
  type LanguageCode,
} from "../lib/i18n";

/**
 * Loads the stored UI language, exposes the translation function, and applies
 * language changes immediately. Outside a Tauri runtime English applies.
 */
export function I18nProvider({ children }: { children: ReactNode }) {
  const [language, setLanguageState] = useState<LanguageCode>("en");

  useEffect(() => {
    let cancelled = false;
    const load = async () => {
      try {
        const view = await fetchSettings();
        const stored = view.settings.language;
        if (!cancelled && LANGUAGES.some((entry) => entry.code === stored)) {
          setLanguageState(stored as LanguageCode);
        }
      } catch {
        // English applies outside a Tauri runtime.
      }
    };
    void load();
    const unlisten = listen<string>("language-changed", (event) => {
      if (LANGUAGES.some((entry) => entry.code === event.payload)) {
        setLanguageState(event.payload as LanguageCode);
      }
    });
    return () => {
      cancelled = true;
      void unlisten.then((fn) => fn());
    };
  }, []);

  const i18n: I18n = useMemo<I18n>(
    () => ({
      language,
      t: (key, vars) => translate(language, key, vars),
    }),
    [language],
  );

  return (
    <I18nContext.Provider value={i18n}>{children}</I18nContext.Provider>
  );
}
