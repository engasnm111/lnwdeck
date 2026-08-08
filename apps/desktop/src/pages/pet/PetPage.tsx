import { useCallback, useEffect, useState } from "react";
import { Badge, Button, Card, DataState, Field, Toggle } from "@lnwdeck/ui";
import {
  fetchPetSpritesheetUrl,
  fetchPetWindowSettings,
  hidePetWindow,
  importWidgetPet,
  listWidgetPets,
  removeWidgetPet,
  setPetAutoSleep,
  setPetCharacter,
  setPetOpacity,
  setPetPose,
  setPetSizePreset,
  setPetSpeed,
  setPetStayInPlace,
  showPetWindow,
  type PetManifest,
  type PetPoseKey,
  type PetSizePreset,
  type PetWindowSettingsData,
} from "../../lib/native";
import { dataStateLabels, useI18n } from "../../lib/i18n";

/** Ambient poses the user can toggle, in UI order. */
const POSE_OPTIONS: Array<{ key: PetPoseKey; labelKey: string; field: keyof PetWindowSettingsData }> = [
  { key: "pet_pose_wave", labelKey: "pose.wave", field: "poseWave" },
  { key: "pet_pose_jump", labelKey: "pose.jump", field: "poseJump" },
  { key: "pet_pose_look_left", labelKey: "pose.lookLeft", field: "poseLookLeft" },
  { key: "pet_pose_look_right", labelKey: "pose.lookRight", field: "poseLookRight" },
  { key: "pet_pose_waiting", labelKey: "pose.waiting", field: "poseWaiting" },
  { key: "pet_pose_review", labelKey: "pose.review", field: "poseReview" },
];

/**
 * Desktop pet page.
 *
 * Controls the floating desktop pet: show/hide, character selection from the
 * installed roster (bundled defaults + community imports), walk speed,
 * opacity and auto-sleep. Every control writes through a validated backend
 * command and reports what was actually stored.
 */
export function PetPage() {
  const { t } = useI18n();
  const [settings, setSettings] = useState<PetWindowSettingsData | null>(null);
  const [pets, setPets] = useState<PetManifest[]>([]);
  const [previews, setPreviews] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);
  const [petImport, setPetImport] = useState("");
  const [petImporting, setPetImporting] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [s, installed] = await Promise.all([
        fetchPetWindowSettings(),
        listWidgetPets(),
      ]);
      setSettings(s);
      setPets(installed);
      // Load a preview (one idle frame) for every installed pet.
      const urls: Record<string, string> = {};
      await Promise.all(
        installed.map(async (pet) => {
          try {
            urls[pet.id] = await fetchPetSpritesheetUrl(pet.id);
          } catch {
            // A broken pet shows its name without a preview.
          }
        }),
      );
      setPreviews(urls);
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

  const reloadSettings = useCallback(async () => {
    try {
      setSettings(await fetchPetWindowSettings());
    } catch (error_) {
      setError(error_ instanceof Error ? error_ : new Error(String(error_)));
    }
  }, []);

  const handleToggleVisible = useCallback(async () => {
    if (!settings) return;
    try {
      if (settings.visible) {
        await hidePetWindow();
      } else {
        await showPetWindow();
      }
      await reloadSettings();
    } catch (error_) {
      setError(error_ instanceof Error ? error_ : new Error(String(error_)));
    }
  }, [settings, reloadSettings]);

  const handleSelectCharacter = useCallback(
    async (id: string) => {
      try {
        await setPetCharacter(id);
        await reloadSettings();
      } catch (error_) {
        setError(error_ instanceof Error ? error_ : new Error(String(error_)));
      }
    },
    [reloadSettings],
  );

  const handleSetSpeed = useCallback(
    async (speed: string) => {
      try {
        await setPetSpeed(speed);
        await reloadSettings();
      } catch (error_) {
        setError(error_ instanceof Error ? error_ : new Error(String(error_)));
      }
    },
    [reloadSettings],
  );

  const handleSetOpacity = useCallback(
    async (opacity: number) => {
      try {
        await setPetOpacity(opacity);
        await reloadSettings();
      } catch (error_) {
        setError(error_ instanceof Error ? error_ : new Error(String(error_)));
      }
    },
    [reloadSettings],
  );

  const handleSetAutoSleep = useCallback(
    async (autoSleep: boolean) => {
      try {
        await setPetAutoSleep(autoSleep);
        await reloadSettings();
      } catch (error_) {
        setError(error_ instanceof Error ? error_ : new Error(String(error_)));
      }
    },
    [reloadSettings],
  );

  const handleSetSize = useCallback(
    async (preset: PetSizePreset) => {
      try {
        await setPetSizePreset(preset);
        await reloadSettings();
      } catch (error_) {
        setError(error_ instanceof Error ? error_ : new Error(String(error_)));
      }
    },
    [reloadSettings],
  );

  const handleSetStayInPlace = useCallback(
    async (stayInPlace: boolean) => {
      try {
        await setPetStayInPlace(stayInPlace);
        await reloadSettings();
      } catch (error_) {
        setError(error_ instanceof Error ? error_ : new Error(String(error_)));
      }
    },
    [reloadSettings],
  );

  const handleSetPose = useCallback(
    async (key: PetPoseKey, enabled: boolean) => {
      try {
        await setPetPose(key, enabled);
        await reloadSettings();
      } catch (error_) {
        setError(error_ instanceof Error ? error_ : new Error(String(error_)));
      }
    },
    [reloadSettings],
  );

  const handleImport = useCallback(async () => {
    if (!petImport.trim()) return;
    setPetImporting(true);
    try {
      await importWidgetPet(petImport.trim());
      setPetImport("");
      await load();
    } catch (error_) {
      setError(error_ instanceof Error ? error_ : new Error(String(error_)));
    } finally {
      setPetImporting(false);
    }
  }, [petImport, load]);

  const handleRemove = useCallback(
    async (id: string) => {
      try {
        await removeWidgetPet(id);
        await load();
      } catch (error_) {
        setError(error_ instanceof Error ? error_ : new Error(String(error_)));
      }
    },
    [load],
  );

  return (
    <div>
      <div className="page-header">
        <div>
          <h2 className="page-title">{t("pet.title")}</h2>
          <p className="page-subtitle">{t("pet.subtitle")}</p>
        </div>
      </div>

      <DataState
        labels={dataStateLabels(t)}
        loading={loading}
        error={error}
        isEmpty={false}
        onRetry={() => void load()}
      >
        {settings && (
          <div className="stack">
            <Card title={t("pet.desktop.title")}>
              <div className="stack-tight">
                <Toggle
                  id="pet-visible"
                  label={t("pet.show.label")}
                  hint={
                    settings.visible
                      ? t("pet.show.hintVisible")
                      : t("pet.show.hintHidden")
                  }
                  checked={settings.visible}
                  onChange={() => void handleToggleVisible()}
                />
                <Field label={t("pet.speed.label")} htmlFor="pet-speed">
                  <select
                    id="pet-speed"
                    className="ui-select"
                    value={settings.speed}
                    onChange={(e) => void handleSetSpeed(e.target.value)}
                  >
                    <option value="slow">{t("pet.speed.slow")}</option>
                    <option value="normal">{t("pet.speed.normal")}</option>
                    <option value="fast">{t("pet.speed.fast")}</option>
                  </select>
                </Field>
                <Toggle
                  id="pet-stay"
                  label={t("pet.stay.label")}
                  hint={
                    settings.stayInPlace
                      ? t("pet.stay.hintOn")
                      : t("pet.stay.hintOff")
                  }
                  checked={settings.stayInPlace}
                  onChange={(checked) => void handleSetStayInPlace(checked)}
                />
                <Field
                  label={t("pet.size.label")}
                  htmlFor="pet-size"
                  hint={t("pet.size.hint")}
                >
                  <select
                    id="pet-size"
                    className="ui-select"
                    value={settings.sizePreset}
                    onChange={(e) =>
                      void handleSetSize(e.target.value as PetSizePreset)
                    }
                  >
                    <option value="small">{t("pet.size.small")}</option>
                    <option value="medium">{t("pet.size.medium")}</option>
                    <option value="large">{t("pet.size.large")}</option>
                  </select>
                </Field>
                <Field
                  label={`${t("pet.opacity.label")}: ${Math.round(settings.opacity * 100)}%`}
                  htmlFor="pet-opacity"
                >
                  <input
                    id="pet-opacity"
                    className="ui-input"
                    type="range"
                    min={10}
                    max={100}
                    step={10}
                    value={Math.round(settings.opacity * 100)}
                    onChange={(e) =>
                      void handleSetOpacity(Number(e.target.value) / 100)
                    }
                  />
                </Field>
                <Toggle
                  id="pet-auto-sleep"
                  label={t("pet.autoSleep.label")}
                  hint={t("pet.autoSleep.hint")}
                  checked={settings.autoSleep}
                  onChange={(checked) => void handleSetAutoSleep(checked)}
                />
                <fieldset className="ui-fieldset">
                  <legend className="ui-fieldset-legend">
                    {t("pet.poses.title")}
                  </legend>
                  <div className="settings-pose-grid">
                    {POSE_OPTIONS.map((option) => (
                      <Toggle
                        key={option.key}
                        id={`pose-${option.key}`}
                        label={t(option.labelKey)}
                        checked={Boolean(settings[option.field])}
                        onChange={(checked) =>
                          void handleSetPose(option.key, checked)
                        }
                      />
                    ))}
                  </div>
                </fieldset>
              </div>
            </Card>

            <Card
              title={t("pet.character")}
              subtitle={t("pet.characterSubtitle", { count: String(pets.length) })}
            >
              <div className="stack-tight">
                {pets.length === 0 ? (
                  <p className="ui-inline-note">
                    {t("pet.noCharacters")}
                  </p>
                ) : (
                  <ul className="settings-pet-list">
                    {pets.map((pet) => (
                      <li
                        key={pet.id}
                        className={`row-between ${
                          settings.character === pet.id
                            ? "pet-card-active"
                            : ""
                        }`.trim()}
                      >
                        <div className="row">
                          {previews[pet.id] ? (
                            <span
                              className="pet-preview"
                              data-sprite-version={pet.spriteVersionNumber || 1}
                              style={{
                                backgroundImage: `url(${previews[pet.id]})`,
                              }}
                              aria-hidden="true"
                            />
                          ) : (
                            <span className="pet-preview" aria-hidden="true" />
                          )}
                          <div className="stack-tight">
                            <span className="meta-value">
                              {pet.displayName}
                            </span>
                            <Badge
                              tone={
                                settings.character === pet.id
                                  ? "success"
                                  : "neutral"
                              }
                            >
                              {settings.character === pet.id
                                ? t("common.active")
                                : t("common.installed")}
                            </Badge>
                          </div>
                        </div>
                        <div className="row">
                          <Button
                            size="small"
                            variant={
                              settings.character === pet.id
                                ? "primary"
                                : "secondary"
                            }
                            disabled={settings.character === pet.id}
                            onClick={() => void handleSelectCharacter(pet.id)}
                          >
                            {settings.character === pet.id
                              ? t("common.active")
                              : t("common.use")}
                          </Button>
                          <Button
                            size="small"
                            variant="danger"
                            onClick={() => void handleRemove(pet.id)}
                          >
                            {t("common.remove")}
                          </Button>
                        </div>
                      </li>
                    ))}
                  </ul>
                )}

                <div className="pet-import-heading">
                  <strong>{t("pet.importTitle")}</strong>
                  <span className="ui-inline-note">{t("pet.importNote")}</span>
                </div>
                <div className="settings-import">
                  <input
                    className="ui-input settings-import-input"
                    type="text"
                    placeholder={t("pet.importPlaceholder")}
                    aria-label={t("pet.importLabel")}
                    value={petImport}
                    disabled={petImporting}
                    onChange={(event) => setPetImport(event.target.value)}
                  />
                  <Button
                    size="small"
                    onClick={() => void handleImport()}
                    disabled={petImporting || petImport.trim() === ""}
                  >
                    {petImporting ? t("common.importing") : t("pet.importButton")}
                  </Button>
                </div>
                <ol className="settings-import-steps">
                  <li>{t("pet.importSteps.step1")}</li>
                  <li>{t("pet.importSteps.step2")}</li>
                  <li>{t("pet.importSteps.step3")}</li>
                </ol>
              </div>
            </Card>
          </div>
        )}
      </DataState>
    </div>
  );
}
