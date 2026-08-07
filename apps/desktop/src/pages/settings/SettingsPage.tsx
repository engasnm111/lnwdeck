import { useCallback, useEffect, useRef, useState } from "react";
import { Badge, Button, Card, DataState, Field, Toggle } from "@lnwdeck/ui";
import {
  deleteProviderKey,
  fetchSettings,
  fetchWidgetSettings,
  getWidgetPet,
  importWidgetPet,
  listWidgetPets,
  removeWidgetPet,
  saveSettings,
  setLanguage,
  setPetAutoSleep,
  setPetOpacity,
  setPetPose,
  setPetSizePreset,
  setPetSpeed,
  setPetStayInPlace,
  setProviderKey,
  setWidgetPet,
  setWidgetSizePreset,
  setWidgetView,
  type AppSettingsData,
  type PetManifest,
  type PetPoseKey,
  type PetSizePreset,
  type SettingsViewData,
  type WidgetSizePreset,
  type WidgetView,
} from "../../lib/native";
import { LANGUAGES, useI18n } from "../../lib/i18n";

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

  const [pets, setPets] = useState<PetManifest[]>([]);
  const [activePetId, setActivePetId] = useState("");
  const [petImport, setPetImport] = useState("");
  const [petImporting, setPetImporting] = useState(false);
  const [petError, setPetError] = useState<string | null>(null);
  const [desktopPetError, setDesktopPetError] = useState<string | null>(null);

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

  // Desktop pet controls apply immediately through pet-specific commands and
  // echo the stored value back into the draft so the page never diverges.
  const petEcho = useCallback((fields: Partial<AppSettingsData>) => {
    setDraft((current) => (current ? { ...current, ...fields } : current));
  }, []);

  const handlePetStayInPlace = useCallback(
    async (stayInPlace: boolean) => {
      try {
        petEcho({ pet_stay_in_place: await setPetStayInPlace(stayInPlace) });
      } catch (error_) {
        setDesktopPetError(error_ instanceof Error ? error_.message : String(error_));
      }
    },
    [petEcho],
  );

  const handlePetPose = useCallback(
    async (key: PetPoseKey, enabled: boolean) => {
      try {
        petEcho({ [key]: await setPetPose(key, enabled) });
      } catch (error_) {
        setDesktopPetError(error_ instanceof Error ? error_.message : String(error_));
      }
    },
    [petEcho],
  );

  const handlePetSpeed = useCallback(
    async (speed: string) => {
      try {
        petEcho({ pet_speed: await setPetSpeed(speed) });
      } catch (error_) {
        setDesktopPetError(error_ instanceof Error ? error_.message : String(error_));
      }
    },
    [petEcho],
  );

  const handlePetSize = useCallback(
    async (preset: PetSizePreset) => {
      try {
        petEcho({ pet_size: await setPetSizePreset(preset) });
      } catch (error_) {
        setDesktopPetError(error_ instanceof Error ? error_.message : String(error_));
      }
    },
    [petEcho],
  );

  const handlePetOpacity = useCallback(
    async (opacity: number) => {
      try {
        petEcho({ pet_opacity: await setPetOpacity(opacity) });
      } catch (error_) {
        setDesktopPetError(error_ instanceof Error ? error_.message : String(error_));
      }
    },
    [petEcho],
  );

  const handlePetAutoSleep = useCallback(
    async (autoSleep: boolean) => {
      try {
        petEcho({ pet_auto_sleep: await setPetAutoSleep(autoSleep) });
      } catch (error_) {
        setDesktopPetError(error_ instanceof Error ? error_.message : String(error_));
      }
    },
    [petEcho],
  );

  const loadPets = useCallback(async () => {
    try {
      const [installed, active] = await Promise.all([
        listWidgetPets(),
        getWidgetPet(),
      ]);
      setPets(installed);
      setActivePetId(active?.id ?? "");
    } catch (error_) {
      setDesktopPetError(error_ instanceof Error ? error_.message : String(error_));
    }
  }, []);

  useEffect(() => {
    void loadPets();
  }, [loadPets]);

  const handleImportPet = useCallback(async () => {
    if (!petImport.trim()) {
      return;
    }
    setPetImporting(true);
    setPetError(null);
    try {
      await importWidgetPet(petImport.trim());
      setPetImport("");
      await loadPets();
    } catch (error_) {
      setPetError(error_ instanceof Error ? error_.message : String(error_));
    } finally {
      setPetImporting(false);
    }
  }, [petImport, loadPets]);

  const handleSelectPet = useCallback(async (petId: string) => {
    setPetError(null);
    try {
      setActivePetId(await setWidgetPet(petId));
    } catch (error_) {
      setPetError(error_ instanceof Error ? error_.message : String(error_));
    }
  }, []);

  const handleRemovePet = useCallback(
    async (petId: string) => {
      setPetError(null);
      try {
        await removeWidgetPet(petId);
        await loadPets();
      } catch (error_) {
        setPetError(error_ instanceof Error ? error_.message : String(error_));
      }
    },
    [loadPets],
  );

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

            <Card title={t("settings.petCard")} subtitle={t("settings.petCardSubtitle")}>
              <div className="stack-tight">
                <Field label={t("settings.petSpeed")} htmlFor="settings-pet-speed">
                  <select
                    id="settings-pet-speed"
                    className="ui-select"
                    value={draft.pet_speed}
                    onChange={(event) => void handlePetSpeed(event.target.value)}
                  >
                    <option value="slow">{t("pet.speed.slow")}</option>
                    <option value="normal">{t("pet.speed.normal")}</option>
                    <option value="fast">{t("pet.speed.fast")}</option>
                  </select>
                </Field>
                <Toggle
                  id="settings-pet-stay"
                  label={t("settings.petStay")}
                  hint={
                    draft.pet_stay_in_place
                      ? t("pet.stay.hintOn")
                      : t("pet.stay.hintOff")
                  }
                  checked={draft.pet_stay_in_place}
                  onChange={(checked) => void handlePetStayInPlace(checked)}
                />
                <Field label={t("settings.petSize")} htmlFor="settings-pet-size" hint={t("settings.petSizeHint")}>
                  <select
                    id="settings-pet-size"
                    className="ui-select"
                    value={draft.pet_size}
                    onChange={(event) =>
                      void handlePetSize(event.target.value as PetSizePreset)
                    }
                  >
                    <option value="small">{t("pet.size.small")}</option>
                    <option value="medium">{t("pet.size.medium")}</option>
                    <option value="large">{t("pet.size.large")}</option>
                  </select>
                </Field>
                <Field
                  label={t("settings.petOpacity", { value: String(Math.round(draft.pet_opacity * 100)) })}
                  htmlFor="settings-pet-opacity"
                >
                  <input
                    id="settings-pet-opacity"
                    className="ui-input"
                    type="range"
                    min={10}
                    max={100}
                    step={10}
                    value={Math.round(draft.pet_opacity * 100)}
                    onChange={(event) =>
                      void handlePetOpacity(Number(event.target.value) / 100)
                    }
                  />
                </Field>
                <Toggle
                  id="settings-pet-autosleep"
                  label={t("settings.petAutoSleep")}
                  checked={draft.pet_auto_sleep}
                  onChange={(checked) => void handlePetAutoSleep(checked)}
                />
                <fieldset className="ui-fieldset">
                  <legend className="ui-fieldset-legend">{t("settings.petPoses")}</legend>
                  <div className="settings-pose-grid">
                    {(
                      [
                        ["pet_pose_wave", "pose.wave"],
                        ["pet_pose_jump", "pose.jump"],
                        ["pet_pose_look_left", "pose.lookLeft"],
                        ["pet_pose_look_right", "pose.lookRight"],
                        ["pet_pose_waiting", "pose.waiting"],
                        ["pet_pose_review", "pose.review"],
                      ] as Array<[PetPoseKey, string]>
                    ).map(([key, labelKey]) => (
                      <Toggle
                        key={key}
                        id={`settings-pose-${key}`}
                        label={t(labelKey)}
                        checked={Boolean(draft[key])}
                        onChange={(checked) => void handlePetPose(key, checked)}
                      />
                    ))}
                  </div>
                </fieldset>
                {desktopPetError && (
                  <p className="ui-field-error" role="alert">
                    {desktopPetError}
                  </p>
                )}
              </div>
            </Card>

            <Card
              title={t("settings.widgetPet")}
              subtitle={t("settings.widgetPetSubtitle")}
            >
              <div className="stack-tight">
                <div className="settings-import">
                  <input
                    className="ui-input settings-import-input"
                    type="text"
                    placeholder={t("pet.importPlaceholder")}
                    aria-label="Codex Pets URL or pet id"
                    value={petImport}
                    disabled={petImporting}
                    onChange={(event) => setPetImport(event.target.value)}
                  />
                  <Button
                    size="small"
                    onClick={() => void handleImportPet()}
                    disabled={petImporting || petImport.trim() === ""}
                  >
                    {petImporting ? t("common.importing") : t("common.import")}
                  </Button>
                </div>
                <ol className="settings-import-steps">
                  <li>{t("pet.importSteps.step1")}</li>
                  <li>{t("pet.importSteps.step2")}</li>
                  <li>{t("pet.importSteps.step3")}</li>
                </ol>
                {pets.length === 0 ? (
                  <p className="ui-inline-note">
                    {t("settings.widgetPetEmpty")}
                  </p>
                ) : (
                  <ul className="settings-pet-list">
                    {pets.map((pet) => (
                      <li key={pet.id} className="row-between">
                        <div className="stack-tight">
                          <span className="meta-value">{pet.displayName}</span>
                          <Badge
                            tone={
                              activePetId === pet.id ? "success" : "neutral"
                            }
                          >
                            {activePetId === pet.id
                              ? t("common.active")
                              : t("common.installed")}
                          </Badge>
                        </div>
                        <div className="row">
                          <Button
                            size="small"
                            variant={activePetId === pet.id ? "primary" : "secondary"}
                            disabled={activePetId === pet.id}
                            onClick={() => void handleSelectPet(pet.id)}
                          >
                            {activePetId === pet.id
                              ? t("common.active")
                              : t("common.use")}
                          </Button>
                          <Button
                            size="small"
                            variant="danger"
                            onClick={() => void handleRemovePet(pet.id)}
                          >
                            {t("common.remove")}
                          </Button>
                        </div>
                      </li>
                    ))}
                  </ul>
                )}
                {petError && (
                  <p className="ui-field-error" role="alert">
                    {petError}
                  </p>
                )}
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
                          aria-label={`${credential.display_name} API key`}
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

            <Card title="Privacy">
              <p className="ui-inline-note">
                lnwdeck reads local provider artifacts read-only and stores token
                counts, timestamps, model identifiers and quota values. Prompts,
                responses, file contents, file names and absolute paths are never
                collected. Provider requests only happen for providers where you
                stored a key.
              </p>
            </Card>
          </div>
        )}
      </DataState>
    </div>
  );
}
