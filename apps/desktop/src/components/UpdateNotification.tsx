import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Button } from "@lnwdeck/ui";
import { checkForUpdate, installUpdate } from "../lib/native";

interface UpdateAvailable {
  version: string | null;
  notes: string | null;
}

interface UpdateProgress {
  downloaded: number;
  total: number | null;
}

type Phase = "idle" | "available" | "installing" | "failed";

/**
 * Update banner.
 *
 * Every state shown here comes from the backend: the availability event, the
 * real download progress, and the sanitized failure code. There is no simulated
 * flow and no success message that is not backed by a completed install.
 */
export function UpdateNotification() {
  const [update, setUpdate] = useState<UpdateAvailable | null>(null);
  const [phase, setPhase] = useState<Phase>("idle");
  const [progress, setProgress] = useState<UpdateProgress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [checkFailure, setCheckFailure] = useState<string | null>(null);
  const [dismissed, setDismissed] = useState(false);

  useEffect(() => {
    const unlisteners: Array<() => void> = [];
    const setup = async () => {
      try {
        unlisteners.push(
          await listen<UpdateAvailable>("update-available", (event) => {
            setUpdate(event.payload);
            setPhase("available");
            setDismissed(false);
          }),
        );
        unlisteners.push(
          await listen<UpdateProgress>("update-progress", (event) => {
            setProgress(event.payload);
          }),
        );
        unlisteners.push(
          await listen<{ code: string }>("update-check-failed", (event) => {
            setCheckFailure(event.payload.code);
          }),
        );
      } catch (listenError) {
        // Outside a Tauri runtime there is no event bus; the banner then only
        // reacts to an explicit check.
        setCheckFailure(null);
        void listenError;
      }
    };
    void setup();
    return () => {
      for (const unlisten of unlisteners) {
        unlisten();
      }
    };
  }, []);

  const handleInstall = useCallback(async () => {
    setPhase("installing");
    setError(null);
    try {
      await installUpdate();
      // On success the backend restarts the application, so this line is only
      // reached if the restart itself was refused.
      setPhase("idle");
    } catch (installError) {
      setError(
        installError instanceof Error
          ? installError.message
          : String(installError),
      );
      setPhase("failed");
    }
  }, []);

  const handleRetryCheck = useCallback(async () => {
    setCheckFailure(null);
    try {
      const result = await checkForUpdate();
      if (result.available) {
        setUpdate({ version: result.version, notes: result.notes });
        setPhase("available");
      } else {
        setUpdate(null);
        setPhase("idle");
      }
    } catch (checkError) {
      setCheckFailure(
        checkError instanceof Error ? checkError.message : String(checkError),
      );
    }
  }, []);

  if (checkFailure && !dismissed) {
    return (
      <div className="banner banner-error" role="alert">
        <span className="banner-body">
          The update check did not complete ({checkFailure}).
        </span>
        <Button size="small" onClick={() => void handleRetryCheck()}>
          Check again
        </Button>
        <button
          type="button"
          className="banner-dismiss"
          onClick={() => setDismissed(true)}
          aria-label="Dismiss update check failure"
        >
          x
        </button>
      </div>
    );
  }

  if (!update || dismissed) {
    return null;
  }

  const percent =
    progress && progress.total && progress.total > 0
      ? Math.round((progress.downloaded / progress.total) * 100)
      : null;

  return (
    <div
      className={`banner ${phase === "failed" ? "banner-error" : ""}`.trim()}
      role="alert"
      aria-live="polite"
    >
      <span className="banner-body">
        {phase === "failed" ? (
          <>Update failed: {error}</>
        ) : phase === "installing" ? (
          <>
            Installing version {update.version}
            {percent !== null ? ` (${percent}%)` : " (downloading)"}
          </>
        ) : (
          <>
            <strong>Version {update.version}</strong> is available.
            {update.notes ? ` ${update.notes.slice(0, 120)}` : ""}
          </>
        )}
      </span>
      {phase !== "installing" && (
        <Button
          variant="primary"
          size="small"
          onClick={() => void handleInstall()}
        >
          {phase === "failed" ? "Try again" : "Install and restart"}
        </Button>
      )}
      <button
        type="button"
        className="banner-dismiss"
        onClick={() => setDismissed(true)}
        aria-label="Dismiss update notification"
      >
        x
      </button>
    </div>
  );
}
