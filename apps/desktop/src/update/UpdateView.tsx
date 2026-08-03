import { useState, useCallback } from "react";

type UpdateState =
  | "idle"
  | "checking"
  | "available"
  | "downloading"
  | "verifying"
  | "ready"
  | "failed";

export function UpdateView() {
  const [updateState, setUpdateState] = useState<UpdateState>("idle");
  const [newVersion, setNewVersion] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const checkForUpdates = useCallback(async () => {
    setUpdateState("checking");
    setError(null);
    try {
      // Simulated update check — real implementation uses Tauri invoke
      setUpdateState("available");
      setNewVersion("0.2.0");
    } catch (e) {
      setError(String(e));
      setUpdateState("failed");
    }
  }, []);

  const startUpdate = useCallback(async () => {
    setUpdateState("downloading");
    try {
      // Simulated download
      setUpdateState("verifying");
      setUpdateState("ready");
    } catch (e) {
      setError(String(e));
      setUpdateState("failed");
    }
  }, []);

  return (
    <div>
      <h2>Updates</h2>

      {updateState === "idle" && (
        <button type="button" onClick={checkForUpdates}>
          Check for Updates
        </button>
      )}

      {updateState === "checking" && <p>Checking for updates...</p>}

      {updateState === "available" && (
        <div role="alert">
          <p>
            New version <strong>v{newVersion}</strong> is available.
          </p>
          <button type="button" onClick={startUpdate}>
            Download Update
          </button>
        </div>
      )}

      {updateState === "downloading" && <p>Downloading update...</p>}
      {updateState === "verifying" && <p>Verifying update signature...</p>}

      {updateState === "ready" && (
        <div role="alert">
          <p>Update ready to install.</p>
          <p>
            <strong>Please restart inwdeck to apply the update.</strong>
          </p>
        </div>
      )}

      {updateState === "failed" && (
        <div role="alert">
          <p>Update failed: {error ?? "Unknown error"}</p>
          <button type="button" onClick={checkForUpdates}>
            Retry
          </button>
        </div>
      )}
    </div>
  );
}
