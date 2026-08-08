import { useCallback, useEffect, useRef, useState } from "react";
import { Badge, Button, Card, DataState, Field, Toggle } from "@lnwdeck/ui";
import {
  deleteProviderKey,
  fetchSettings,
  fetchWidgetSettings,
  saveSettings,
  setLanguage,
  setProviderKey,
  setWidgetSizePreset,
  setWidgetView,
  type AppSettingsData,
  type SettingsViewData,
  type WidgetSizePreset,
  type WidgetView,
} from "../../lib/native";
import { dataStateLabels, LANGUAGES, useI18n } from "../../lib/i18n";

function intervalLabel(
  t: (key: string, vars?: Record<string, string>) => string,
  seconds: number,
): string {
  if (seconds === 0) {
    return t("settings.intervalDisabled");
  }
  if (seconds < 60) {
    return t("settings.intervalSeconds", { value: String(seconds) });
  }
  if (seconds < 3600) {
    return t("settings.intervalMinutes", { value: String(seconds / 60) });
  }
  return t("settings.intervalHours", { value: String(seconds / 3600) });
}

function retentionLabel(
  t: (key: string, vars?: Record<string, string>) => string,
  days: number,
): string {
  return days === 0
    ? t("settings.retentionForever")
    : t("settings.retentionDays", { value: String(days) });
}

/**
 * Settings.
 *
 * Every control is bound to state read from the backend and every change is
 * written through a command that validates it. The page reports what was
 * actually stored, so a control cannot show a preference the application does
 * not hold.
 */
export function SettingsPage() {
  const { language, t } = useI18n();
  const [view, setView] = useState<SettingsViewData | null>(null);
  const [draft, setDraft] = useState<AppSettingsData | null>(null);
  const draftRef = useRef<AppSettingsData | null>(null);
  draftRef.current = draft;
  const saveSeq = useRef(0);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [keyDrafts, setKeyDrafts] = useState<Record<string, string>>({});
  const [keyError, setKeyError] = useState<string | null>(null);

  // Widget layout (bars / rings / pet), applied immediately via its own
  // command rather than through the saved-settings form.
  const [widgetView, setWidgetViewState] = useState<WidgetView>("bars");
  const [widgetViewError, setWidgetViewError] = useState<string | null>(null);

  const loadWidgetView = useCallback(async () => {
    try {
      const settings = await fetchWidgetSettings();
      setWidgetViewState(
        settings.view === "rings" || settings.view === "pet"
          ? settings.view
          : "bars",
      );
    } catch {
      // The stored view still applies; the select just shows the default.
    }
  }, []);

  useEffect(() => {
    void loadWidgetView();
  }, [loadWidgetView]);

  const handleWidgetViewChange = useCallback(async (view: WidgetView) => {
    setWidgetViewError(null);
    try {
      const stored = await setWidgetView(view);
      setWidgetViewState(
        stored === "rings" || stored === "pet" ? stored : "bars",
      );
    } catch (error_) {
      setWidgetViewError(error_ instanceof Error ? error_.message : String(error_));
    }
  }, []);

  const handleWidgetSizeChange = useCallback(async (preset: WidgetSizePreset) => {
    setWidgetViewError(null);
    try {
      const stored = await setWidgetSizePreset(preset);
      setDraft((current) =>
        current ? { ...current, widget_size: stored } : current,
      );
    } catch (error_) {
      setWidgetViewError(error_ instanceof Error ? error_.message : String(error_));
    }
  }, []);

  const handleLanguageChange = useCallback(async (code: string) => {
    try {
      const stored = await setLanguage(code);
      setDraft((current) =>
        current ? { ...current, language: stored } : current,
      );
    } catch (error_) {
      setSaveError(error_ instanceof Error ? error_.message : String(error_));
    }
  }, []);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await fetchSettings();
      setView(result);
      setDraft(result.settings);
    } catch (loadError) {
      setError(
        loadError instanceof Error ? loadError : new Error(String(loadError)),
      );
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  // Every control saves immediately through the backend command; there is no
  // save button. The echoed value replaces the draft so a rejected value can
  // never stay visible. A sequence id ignores stale responses when several
  // controls change in quick succession.
  const persist = useCallback(async (next: AppSettingsData) => {
    const seq = ++saveSeq.current;
    setSaveError(null);
    try {
      const stored = await saveSettings(next);
      if (seq === saveSeq.current) {
        setView(stored);
        setDraft(stored.settings);
      }
    } catch (error_) {
      if (seq === saveSeq.current) {
        setSaveError(error_ instanceof Error ? error_.message : String(error_));
      }
    }
  }, []);

  const update = useCallback(
    <K extends keyof AppSettingsData>(key: K, value: AppSettingsData[K]) => {
      const next = draftRef.current
        ? { ...draftRef.current, [key]: value }
        : null;
      if (!next) return;
      setDraft(next);
      void persist(next);
    },
    [persist],
  );

  // The opacity sliders fire many changes while dragging: keep the local
  // draft live but only persist after the user settles.
  const debouncedPersist = useRef<ReturnType<typeof setTimeout> | null>(null);
  const persistSoon = useCallback((next: AppSettingsData) => {
    if (debouncedPersist.current) clearTimeout(debouncedPersist.current);
    debouncedPersist.current = setTimeout(() => {
      void persist(next);
    }, 250);
  }, [persist]);

  const updateSoon = useCallback(
    <K extends keyof AppSettingsData>(key: K, value: AppSettingsData[K]) => {
      const next = draftRef.current
        ? { ...draftRef.current, [key]: value }
        : null;
      if (!next) return;
      setDraft(next);
      persistSoon(next);
    },
    [persistSoon],
  );

  const handleStoreKey = useCallback(
    async (providerId: string) => {
      setKeyError(null);
      try {
        const stored = await setProviderKey(
          providerId,
          keyDrafts[providerId] ?? "",
        );
        setView(stored);
        setKeyDrafts((current) => ({ ...current, [providerId]: "" }));
      } catch (error_) {
        setKeyError(error_ instanceof Error ? error_.message : String(error_));
      }
    },
    [keyDrafts],
  );

  const handleDeleteKey = useCallback(async (providerId: string) => {
    setKeyError(null);
    try {
      setView(await deleteProviderKey(providerId));
    } catch (error_) {
      setKeyError(error_ instanceof Error ? error_.message : String(error_));
    }
  }, []);

  return (
    <div>
      <div className="page-header">
        <div>
          <h2 className="page-title">{t("settings.title")}</h2>
          <p className="page-subtitle">{t("settings.subtitle")}</p>
        </div>
      </div>

      <DataState
        labels={dataStateLabels(t)}
        loading={loading}
        error={error}
        isEmpty={false}
        onRetry={() => void load()}
      >
        {view && draft && (
          <div className="stack">
            <Card title={t("settings.collection")}>
              <div className="settings-grid">
                <Field
                  label={t("settings.autoRefresh")}
                  htmlFor="refresh-interval"
                  hint={t("settings.autoRefreshHint")}
                >
                  <select
                    id="refresh-interval"
                    className="ui-select"
                    value={String(draft.refresh_interval_seconds)}
                    onChange={(event) =>
                      update(
                        "refresh_interval_seconds",
                        Number(event.target.value),
                      )
                    }
                  >
                    {view.allowed_refresh_intervals.map((seconds) => (
                      <option key={seconds} value={seconds}>
                        {intervalLabel(t, seconds)}
                      </option>
                    ))}
                  </select>
                </Field>
                <Field
                  label={t("settings.keepHistory")}
                  htmlFor="retention"
                  hint={t("settings.keepHistoryHint")}
                >
                  <select
                    id="retention"
                    className="ui-select"
                    value={String(draft.retention_days)}
                    onChange={(event) =>
                      update("retention_days", Number(event.target.value))
                    }
                  >
                    {view.allowed_retention_days.map((days) => (
                      <option key={days} value={days}>
                        {retentionLabel(t, days)}
                      </option>
                    ))}
                  </select>
                </Field>
                <Field label={t("settings.theme")} htmlFor="theme">
                  <select
                    id="theme"
                    className="ui-select"
                    value={draft.theme}
                    onChange={(event) =>
                      update(
                        "theme",
                        event.target.value as AppSettingsData["theme"],
                      )
                    }
                  >
                    {view.allowed_themes.map((theme) => (
                      <option key={theme} value={theme}>
                        {theme}
                      </option>
                    ))}
                  </select>
                </Field>
                <Field
                  label={t("settings.language")}
                  htmlFor="language"
                  hint={t("settings.languageHint")}
                >
                  <select
                    id="language"
                    className="ui-select"
                    value={language}
                    onChange={(event) => void handleLanguageChange(event.target.value)}
                  >
                    {LANGUAGES.map((entry) => (
                      <option key={entry.code} value={entry.code}>
                        {entry.nativeName}
                      </option>
                    ))}
                  </select>
                </Field>
              </div>
              {saveError && (
                <p className="ui-field-error" role="alert">
                  {saveError}
                </p>
              )}
            </Card>

            <Card title={t("settings.windows")}>
              <div className="stack-tight">
                <Toggle
                  id="launch-at-startup"
                  label={t("settings.startup")}
                  hint={
                    view.startup_supported
                      ? t("settings.startupState", {
                          state: view.startup_registered
                            ? t("settings.startupPresent")
                            : t("settings.startupAbsent"),
                        })
                      : t("settings.startupUnsupported")
                  }
                  checked={draft.launch_at_startup}
                  disabled={!view.startup_supported}
                  onChange={(checked) => update("launch_at_startup", checked)}
                />
                <Toggle
                  id="auto-update"
                  label={t("settings.autoUpdate")}
                  hint={t("settings.autoUpdateHint")}
                  checked={draft.auto_update_check}
                  onChange={(checked) => update("auto_update_check", checked)}
                />
                <Toggle
                  id="widget-visible"
                  label={t("settings.widgetVisible")}
                  checked={draft.widget_visible}
                  onChange={(checked) => update("widget_visible", checked)}
                />
                <Toggle
                  id="widget-locked"
                  label={t("settings.widgetLocked")}
                  hint={t("settings.widgetLockedHint")}
                  checked={draft.widget_locked}
                  onChange={(checked) => update("widget_locked", checked)}
                />
                <Field
                  label={t("settings.widgetSize")}
                  htmlFor="widget-size"
                  hint={t("settings.widgetSizeHint")}
                >
                  <select
                    id="widget-size"
                    className="ui-select"
                    value={draft.widget_size}
                    onChange={(event) =>
                      void handleWidgetSizeChange(
                        event.target.value as WidgetSizePreset,
                      )
                    }
                  >
                    <option value="small">{t("settings.widgetSizeSmall")}</option>
                    <option value="medium">{t("settings.widgetSizeMedium")}</option>
                    <option value="large">{t("settings.widgetSizeLarge")}</option>
                  </select>
                </Field>
                <Field
                  label={t("settings.widgetLayout")}
                  htmlFor="widget-layout"
                  hint={t("settings.widgetLayoutHint")}
                >
                  <select
                    id="widget-layout"
                    className="ui-select"
                    value={widgetView}
                    onChange={(event) =>
                      void handleWidgetViewChange(event.target.value as WidgetView)
                    }
                  >
                    <option value="bars">{t("settings.widgetViewBars")}</option>
                    <option value="rings">{t("settings.widgetViewRings")}</option>
                    <option value="pet">{t("settings.widgetViewPet")}</option>
                  </select>
                  {widgetViewError && (
                    <p className="ui-field-error" role="alert">
                      {widgetViewError}
                    </p>
                  )}
                </Field>
                <Field
                  label={t("settings.widgetOpacity", { value: String(Math.round(draft.widget_opacity * 100)) })}
                  htmlFor="widget-opacity"
                >
                  <input
                    id="widget-opacity"
                    className="ui-input"
                    type="range"
                    min={10}
                    max={100}
                    step={10}
                    value={Math.round(draft.widget_opacity * 100)}
                    onChange={(event) =>
                      updateSoon("widget_opacity", Number(event.target.value) / 100)
                    }
                  />
                </Field>
              </div>
            </Card>

            <Card
              title={t("settings.keys.title")}
              subtitle={
                view.credential_store_supported
                  ? t("settings.keys.storedHint")
                  : t("settings.keys.noStore")
              }
            >
              {view.provider_credentials.length === 0 ? (
                <p className="ui-inline-note">
                  {t("settings.keys.none")}
                </p>
              ) : (
                <div className="stack-tight">
                  {view.provider_credentials.map((credential) => (
                    <div key={credential.provider_id} className="row-between">
                      <div className="stack-tight">
                        <span className="meta-value">
                          {credential.display_name}
                        </span>
                        <Badge
                          tone={
                            credential.state === "configured"
                              ? "success"
                              : credential.state === "expired"
                                ? "warning"
                                : "neutral"
                          }
                        >
                          {credential.state}
                        </Badge>
                      </div>
                      <div className="row">
                        <input
                          className="ui-input"
                          type="password"
                          placeholder={t("settings.keys.placeholder")}
                          aria-label={t("settings.apiKeyAria", { provider: credential.display_name })}
                          value={keyDrafts[credential.provider_id] ?? ""}
                          disabled={!view.credential_store_supported}
                          onChange={(event) =>
                            setKeyDrafts((current) => ({
                              ...current,
                              [credential.provider_id]: event.target.value,
                            }))
                          }
                        />
                        <Button
                          size="small"
                          disabled={!view.credential_store_supported}
                          onClick={() =>
                            void handleStoreKey(credential.provider_id)
                          }
                        >
                          {t("settings.keys.store")}
                        </Button>
                        <Button
                          size="small"
                          variant="danger"
                          disabled={credential.state === "missing"}
                          onClick={() =>
                            void handleDeleteKey(credential.provider_id)
                          }
                        >
                          {t("common.remove")}
                        </Button>
                      </div>
                    </div>
                  ))}
                </div>
              )}
              {keyError && (
                <p className="ui-field-error" role="alert">
                  {keyError}
                </p>
              )}
            </Card>

            <Card title={t("settings.privacyTitle")}>
              <p className="ui-inline-note">
                {t("settings.privacyBody")}
              </p>
            </Card>
          </div>
        )}
      </DataState>
    </div>
  );
}
