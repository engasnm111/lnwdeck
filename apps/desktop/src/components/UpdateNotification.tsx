import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

interface UpdatePayload {
  version: string;
  body: string;
}

interface UpdateProgress {
  downloaded: number;
  total: number;
}

/**
 * Notification banner shown when a new update is available.
 * Listens for the `update-available` event emitted by the Rust backend.
 */
export function UpdateNotification() {
  const [update, setUpdate] = useState<UpdatePayload | null>(null);
  const [installing, setInstalling] = useState(false);
  const [progress, setProgress] = useState<UpdateProgress | null>(null);
  const [result, setResult] = useState<string | null>(null);
  const [dismissed, setDismissed] = useState(false);

  useEffect(() => {
    let unlistenUpdate: (() => void) | undefined;
    let unlistenProgress: (() => void) | undefined;

    const setup = async () => {
      try {
        unlistenUpdate = await listen<UpdatePayload>(
          "update-available",
          (event) => {
            setUpdate(event.payload);
          },
        );
        unlistenProgress = await listen<UpdateProgress>(
          "update-progress",
          (event) => {
            setProgress(event.payload);
          },
        );
      } catch {
        // Not in Tauri environment — skip
      }
    };

    setup();

    return () => {
      unlistenUpdate?.();
      unlistenProgress?.();
    };
  }, []);

  if (!update || dismissed) return null;

  const handleInstall = async () => {
    setInstalling(true);
    try {
      const msg = await invoke<string>("check_for_update");
      setResult(msg);
    } catch (err) {
      setResult(`Update failed: ${err}`);
    } finally {
      setInstalling(false);
    }
  };

  const progressPercent =
    progress && progress.total > 0
      ? Math.round((progress.downloaded / progress.total) * 100)
      : null;

  return (
    <div
      role="alert"
      aria-live="polite"
      style={{
        display: "flex",
        alignItems: "center",
        gap: "0.75rem",
        padding: "0.625rem 1rem",
        background: "var(--color-info-bg, #1a2744)",
        borderBottom: "1px solid var(--color-info-border, #2a4a7f)",
        fontSize: "0.8125rem",
        color: "var(--text-primary, #e1e1e6)",
      }}
    >
      <span style={{ fontSize: "1rem" }}>🔄</span>

      {result ? (
        <span style={{ flex: 1 }}>{result}</span>
      ) : (
        <span style={{ flex: 1 }}>
          <strong>lnwdeck v{update.version}</strong> is available.
          {update.body && (
            <span style={{ color: "var(--text-muted)", marginLeft: "0.5rem" }}>
              {update.body.length > 120
                ? `${update.body.slice(0, 120)}…`
                : update.body}
            </span>
          )}
        </span>
      )}

      {installing && progressPercent !== null && (
        <span style={{ color: "var(--text-muted)", minWidth: "3rem" }}>
          {progressPercent}%
        </span>
      )}

      {!result && (
        <button
          type="button"
          onClick={handleInstall}
          disabled={installing}
          style={{
            padding: "0.25rem 0.75rem",
            borderRadius: "4px",
            border: "1px solid var(--color-accent, #6f7df6)",
            background: "var(--color-accent, #6f7df6)",
            color: "#fff",
            cursor: installing ? "wait" : "pointer",
            fontSize: "0.75rem",
            fontWeight: 600,
            opacity: installing ? 0.7 : 1,
          }}
        >
          {installing ? "Installing…" : "Update Now"}
        </button>
      )}

      <button
        type="button"
        onClick={() => setDismissed(true)}
        aria-label="Dismiss update notification"
        style={{
          background: "transparent",
          border: "none",
          color: "var(--text-muted)",
          cursor: "pointer",
          fontSize: "1rem",
          padding: "0 0.25rem",
        }}
      >
        ✕
      </button>
    </div>
  );
}
