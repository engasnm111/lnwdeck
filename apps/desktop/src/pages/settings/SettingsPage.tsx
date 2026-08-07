import { useCallback, useEffect, useState } from "react";
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
  setProviderKey,
  setWidgetPet,
  setWidgetSizePreset,
  setWidgetView,
  type AppSettingsData,
  type PetManifest,
  type SettingsViewData,
  type WidgetSizePreset,
  type WidgetView,
} from "../../lib/native";

function intervalLabel(seconds: number): string {
  if (seconds === 0) {
    return "Disabled";
  }
  if (seconds < 60) {
    return `${seconds} seconds`;
  }
  if (seconds < 3600) {
    return `${seconds / 60} minutes`;
  }
  return `${seconds / 3600} hour(s)`;
}

function retentionLabel(days: number): string {
  return days === 0 ? "Keep everything" : `${days} days`;
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
  const [view, setView] = useState<SettingsViewData | null>(null);
  const [draft, setDraft] = useState<AppSettingsData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [savedAt, setSavedAt] = useState<string | null>(null);
  const [keyDrafts, setKeyDrafts] = useState<Record<string, string>>({});
  const [keyError, setKeyError] = useState<string | null>(null);

  const [pets, setPets] = useState<PetManifest[]>([]);
  const [activePetId, setActivePetId] = useState("");
  const [petImport, setPetImport] = useState("");
  const [petImporting, setPetImporting] = useState(false);
  const [petError, setPetError] = useState<string | null>(null);

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
      setSavedAt(null);
    } catch (error_) {
      setWidgetViewError(error_ instanceof Error ? error_.message : String(error_));
    }
  }, []);

  const loadPets = useCallback(async () => {
    try {
      const [installed, active] = await Promise.all([
        listWidgetPets(),
        getWidgetPet(),
      ]);
      setPets(installed);
      setActivePetId(active?.id ?? "");
    } catch (error_) {
      setPetError(error_ instanceof Error ? error_.message : String(error_));
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

  const update = useCallback(
    <K extends keyof AppSettingsData>(key: K, value: AppSettingsData[K]) => {
      setDraft((current) => (current ? { ...current, [key]: value } : current));
      setSavedAt(null);
    },
    [],
  );

  const handleSave = useCallback(async () => {
    if (!draft) {
      return;
    }
    setSaving(true);
    setSaveError(null);
    try {
      const stored = await saveSettings(draft);
      setView(stored);
      setDraft(stored.settings);
      setSavedAt(new Date().toLocaleTimeString());
    } catch (error_) {
      setSaveError(error_ instanceof Error ? error_.message : String(error_));
    } finally {
      setSaving(false);
    }
  }, [draft]);

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
          <h2 className="page-title">Settings</h2>
          <p className="page-subtitle">
            Preferences are stored locally and applied by the backend. API keys
            go to the Windows Credential Manager, never to the database.
          </p>
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
            <Card
              title="Collection"
              action={
                <Button
                  variant="primary"
                  onClick={() => void handleSave()}
                  disabled={saving}
                >
                  {saving ? "Saving" : "Save settings"}
                </Button>
              }
            >
              <div className="settings-grid">
                <Field
                  label="Automatic refresh"
                  htmlFor="refresh-interval"
                  hint="How often providers are collected in the background"
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
                        {intervalLabel(seconds)}
                      </option>
                    ))}
                  </select>
                </Field>
                <Field
                  label="Keep history for"
                  htmlFor="retention"
                  hint="Older records are pruned"
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
                        {retentionLabel(days)}
                      </option>
                    ))}
                  </select>
                </Field>
                <Field label="Theme" htmlFor="theme">
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
              </div>
              {saveError && (
                <p className="ui-field-error" role="alert">
                  {saveError}
                </p>
              )}
              {savedAt && (
                <p className="ui-inline-note" role="status">
                  Saved at {savedAt}
                </p>
              )}
            </Card>

            <Card title="Windows integration">
              <div className="stack-tight">
                <Toggle
                  id="launch-at-startup"
                  label="Start lnwdeck when Windows starts"
                  hint={
                    view.startup_supported
                      ? `Registry entry currently ${view.startup_registered ? "present" : "absent"}`
                      : "Not supported on this platform"
                  }
                  checked={draft.launch_at_startup}
                  disabled={!view.startup_supported}
                  onChange={(checked) => update("launch_at_startup", checked)}
                />
                <Toggle
                  id="auto-update"
                  label="Check for updates automatically"
                  hint="A failed check is reported, never hidden"
                  checked={draft.auto_update_check}
                  onChange={(checked) => update("auto_update_check", checked)}
                />
                <Toggle
                  id="widget-visible"
                  label="Show the floating quota widget"
                  checked={draft.widget_visible}
                  onChange={(checked) => update("widget_visible", checked)}
                />
                <Toggle
                  id="widget-locked"
                  label="Lock the widget in place"
                  hint="A locked widget cannot be dragged"
                  checked={draft.widget_locked}
                  onChange={(checked) => update("widget_locked", checked)}
                />
                <Field
                  label="Widget size"
                  htmlFor="widget-size"
                  hint="The widget is fixed-size; content scrolls inside it"
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
                    <option value="small">Small (300 x 300)</option>
                    <option value="medium">Medium (400 x 420)</option>
                    <option value="large">Large (500 x 500)</option>
                  </select>
                </Field>
                <Field
                  label="Widget layout"
                  htmlFor="widget-layout"
                  hint="Bars stack as rows; rings wrap to fit the size"
                >
                  <select
                    id="widget-layout"
                    className="ui-select"
                    value={widgetView}
                    onChange={(event) =>
                      void handleWidgetViewChange(event.target.value as WidgetView)
                    }
                  >
                    <option value="bars">Bars</option>
                    <option value="rings">Rings</option>
                    <option value="pet">Pet</option>
                  </select>
                  {widgetViewError && (
                    <p className="ui-field-error" role="alert">
                      {widgetViewError}
                    </p>
                  )}
                </Field>
                <Field
                  label={`Widget opacity: ${Math.round(draft.widget_opacity * 100)}%`}
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
                      update("widget_opacity", Number(event.target.value) / 100)
                    }
                  />
                </Field>
              </div>
            </Card>

            <Card
              title="Widget pet"
              subtitle="Community pets from codex-pets.net, downloaded once and rendered locally"
            >
              <div className="stack-tight">
                <div className="row">
                  <input
                    className="ui-input"
                    type="text"
                    placeholder="Pet id or https://codex-pets.net pet URL"
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
                    {petImporting ? "Importing" : "Import"}
                  </Button>
                </div>
                {pets.length === 0 ? (
                  <p className="ui-inline-note">
                    No community pets installed; the built-in robot is always
                    available. Imports only reach codex-pets.net over HTTPS on
                    your explicit action.
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
                            {activePetId === pet.id ? "Active" : "Installed"}
                          </Badge>
                        </div>
                        <div className="row">
                          <Button
                            size="small"
                            variant={activePetId === pet.id ? "primary" : "secondary"}
                            disabled={activePetId === pet.id}
                            onClick={() => void handleSelectPet(pet.id)}
                          >
                            {activePetId === pet.id ? "Active" : "Use"}
                          </Button>
                          <Button
                            size="small"
                            variant="danger"
                            onClick={() => void handleRemovePet(pet.id)}
                          >
                            Remove
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
              title="Provider API keys"
              subtitle={
                view.credential_store_supported
                  ? "Stored in the Windows Credential Manager"
                  : "This platform has no credential store, so keys cannot be stored"
              }
            >
              {view.provider_credentials.length === 0 ? (
                <p className="ui-inline-note">
                  No registered provider requires an API key.
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
                          placeholder="API key"
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
                          Store
                        </Button>
                        <Button
                          size="small"
                          variant="danger"
                          disabled={credential.state === "missing"}
                          onClick={() =>
                            void handleDeleteKey(credential.provider_id)
                          }
                        >
                          Remove
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
