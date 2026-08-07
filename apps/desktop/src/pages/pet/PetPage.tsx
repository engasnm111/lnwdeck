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
  setPetSizePreset,
  setPetSpeed,
  showPetWindow,
  type PetManifest,
  type PetSizePreset,
  type PetWindowSettingsData,
} from "../../lib/native";

/**
 * Desktop pet page.
 *
 * Controls the floating desktop pet: show/hide, character selection from the
 * installed roster (bundled defaults + community imports), walk speed,
 * opacity and auto-sleep. Every control writes through a validated backend
 * command and reports what was actually stored.
 */
export function PetPage() {
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
          <h2 className="page-title">Pet</h2>
          <p className="page-subtitle">
            A floating companion that walks across your screen and shows your
            token usage when hovered. Right-click the pet to close it.
          </p>
        </div>
      </div>

      <DataState
        loading={loading}
        error={error}
        isEmpty={false}
        onRetry={() => void load()}
      >
        {settings && (
          <div className="stack">
            <Card title="Desktop pet">
              <div className="stack-tight">
                <Toggle
                  id="pet-visible"
                  label="Show desktop pet"
                  hint={
                    settings.visible
                      ? "The pet is walking on your screen now"
                      : "The pet appears near the bottom of the screen"
                  }
                  checked={settings.visible}
                  onChange={() => void handleToggleVisible()}
                />
                <Field label="Walk speed" htmlFor="pet-speed">
                  <select
                    id="pet-speed"
                    className="ui-select"
                    value={settings.speed}
                    onChange={(e) => void handleSetSpeed(e.target.value)}
                  >
                    <option value="slow">Slow</option>
                    <option value="normal">Normal</option>
                    <option value="fast">Fast</option>
                  </select>
                </Field>
                <Field
                  label="Pet size"
                  htmlFor="pet-size"
                  hint="The pet window is fixed-size; the sprite scales with it"
                >
                  <select
                    id="pet-size"
                    className="ui-select"
                    value={settings.size_preset}
                    onChange={(e) =>
                      void handleSetSize(e.target.value as PetSizePreset)
                    }
                  >
                    <option value="small">Small (200 x 300)</option>
                    <option value="medium">Medium (280 x 400)</option>
                    <option value="large">Large (360 x 520)</option>
                  </select>
                </Field>
                <Field
                  label={`Opacity: ${Math.round(settings.opacity * 100)}%`}
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
                  label="Auto-sleep after inactivity"
                  hint="The pet falls asleep when you stop interacting"
                  checked={settings.auto_sleep}
                  onChange={(checked) => void handleSetAutoSleep(checked)}
                />
              </div>
            </Card>

            <Card
              title="Characters"
              subtitle="Six defaults are bundled from codex-pets.net; community pets can be imported by id or URL"
            >
              <div className="stack-tight">
                {pets.length === 0 ? (
                  <p className="ui-inline-note">
                    No characters installed yet. Import one from codex-pets.net
                    below.
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
                                ? "Active"
                                : "Installed"}
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
                            {settings.character === pet.id ? "Active" : "Use"}
                          </Button>
                          <Button
                            size="small"
                            variant="danger"
                            onClick={() => void handleRemove(pet.id)}
                          >
                            Remove
                          </Button>
                        </div>
                      </li>
                    ))}
                  </ul>
                )}

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
                    onClick={() => void handleImport()}
                    disabled={petImporting || petImport.trim() === ""}
                  >
                    {petImporting ? "Importing" : "Import"}
                  </Button>
                </div>
                <p className="ui-inline-note">
                  Imports only reach codex-pets.net over HTTPS on your explicit
                  action and are stored locally.
                </p>
              </div>
            </Card>
          </div>
        )}
      </DataState>
    </div>
  );
}
