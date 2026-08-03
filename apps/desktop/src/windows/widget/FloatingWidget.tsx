import { useCallback, useEffect, useRef, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  fetchQuotaDashboard,
  hideWidgetWindow,
  showMainWindow,
  type ProviderQuotaCard,
  type QuotaDashboardData,
  type QuotaWindowData,
} from "../../lib/native";
import { formatCompact, formatCountdown, formatRefreshedAgo } from "./widgetTime";

type LockMode = "unlocked" | "locked";

interface WidgetState {
  opacity: number;
  lockMode: LockMode;
}

const STORAGE_KEY = "lnwdeck_widget_state";

function loadWidgetState(): WidgetState {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored) {
      const parsed = JSON.parse(stored) as Partial<WidgetState>;
      return {
        opacity: clampOpacity(parsed.opacity ?? 1.0),
        lockMode: parsed.lockMode === "locked" ? "locked" : "unlocked",
      };
    }
  } catch {
    // ignore corrupt state
  }
  return { opacity: 1.0, lockMode: "unlocked" };
}

function saveWidgetState(state: WidgetState) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  } catch {
    // ignore quota-limit storage errors
  }
}

function clampOpacity(value: number): number {
  return Math.max(0.1, Math.min(1.0, value));
}

const STATUS_LABELS: Record<ProviderQuotaCard["status"], string> = {
  fresh: "OK",
  stale: "stale",
  unavailable: "unavailable",
  auth_expired: "auth expired",
  rate_limited: "rate limited",
  error: "error",
};

function WindowRow({ window, now }: { window: QuotaWindowData; now: number }) {
  if (window.is_unlimited) {
    return (
      <div className="widget-window">
        <span className="widget-window-metric">Local / Unlimited</span>
      </div>
    );
  }

  const countdown = formatCountdown(window.reset_at, now);
  if (window.limit === 0) {
    return (
      <div className="widget-window">
        <span className="widget-window-metric">
          used {formatCompact(window.used)} {window.kind}
          <span className="widget-estimate"> estimate</span>
        </span>
      </div>
    );
  }

  const remainingPercent = Math.round(window.remaining_percent);
  return (
    <div className="widget-window">
      <div className="widget-bar" aria-hidden="true">
        <div
          className="widget-bar-fill"
          style={{ width: `${window.remaining_percent}%` }}
        />
      </div>
      <span className="widget-window-metric">
        {remainingPercent}% left
        {countdown ? ` · resets ${countdown}` : ""}
      </span>
    </div>
  );
}

function ProviderRow({
  provider,
  now,
}: {
  provider: ProviderQuotaCard;
  now: number;
}) {
  const isError = provider.status !== "fresh" && provider.status !== "stale";
  return (
    <li className={`widget-provider widget-provider-${provider.status}`}>
      <div className="widget-provider-head">
        <span className="widget-provider-name">{provider.display_name}</span>
        {isError ? (
          <span className="widget-status widget-status-error">
            {STATUS_LABELS[provider.status]}
            {provider.error_code ? ` (${provider.error_code})` : ""}
          </span>
        ) : provider.status === "stale" ? (
          <span className="widget-status widget-status-stale">stale</span>
        ) : (
          <span className="widget-status widget-status-ok">OK</span>
        )}
      </div>
      {provider.windows.length === 0 ? (
        <div className="widget-window">
          <span className="widget-window-metric">no quota data</span>
        </div>
      ) : (
        provider.windows.map((w) => <WindowRow key={w.window_key} window={w} now={now} />)
      )}
    </li>
  );
}

export function FloatingWidget() {
  const [dashboard, setDashboard] = useState<QuotaDashboardData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [now, setNow] = useState(Date.now());
  const [state, setState] = useState<WidgetState>(loadWidgetState);
  const unlistenRef = useRef<UnlistenFn | null>(null);

  const load = useCallback(async () => {
    setError(null);
    try {
      const result = await fetchQuotaDashboard();
      setDashboard(result);
    } catch (e) {
      setError(e instanceof Error ? e.message : "quota unavailable");
    }
  }, []);

  useEffect(() => {
    load();

    const interval = setInterval(() => {
      setNow(Date.now());
      void load();
    }, 30_000);
    const countdownTick = setInterval(() => setNow(Date.now()), 1_000);

    void listen("quota-updated", () => void load()).then(
      (unlisten) => {
        unlistenRef.current = unlisten;
      },
      () => {
        // event bus unavailable outside a Tauri runtime
      },
    );

    return () => {
      clearInterval(interval);
      clearInterval(countdownTick);
      unlistenRef.current?.();
    };
  }, [load]);

  const toggleLock = () => {
    const newState = {
      ...state,
      lockMode: (state.lockMode === "locked" ? "unlocked" : "locked") as LockMode,
    };
    setState(newState);
    saveWidgetState(newState);
  };

  const changeOpacity = (delta: number) => {
    const newState = {
      ...state,
      opacity: clampOpacity(state.opacity + delta),
    };
    setState(newState);
    saveWidgetState(newState);
  };

  const hasAnyProvider = (dashboard?.providers.length ?? 0) > 0;

  return (
    <div
      className="widget-root"
      data-tauri-drag-region={state.lockMode === "unlocked" ? "" : undefined}
      style={{ opacity: state.opacity }}
    >
      <header className="widget-header" data-tauri-drag-region="">
        <span className="widget-title" data-tauri-drag-region="">
          lnwdeck
        </span>
        <span className="widget-refreshed">
          {dashboard ? formatRefreshedAgo(dashboard.generated_at, now) : ""}
        </span>
      </header>

      <div className="widget-controls">
        <button
          type="button"
          className="widget-button"
          onClick={toggleLock}
          aria-label={state.lockMode === "locked" ? "Unlock widget" : "Lock widget"}
        >
          {state.lockMode === "locked" ? "unlock" : "lock"}
        </button>
        <button
          type="button"
          className="widget-button"
          onClick={() => changeOpacity(-0.1)}
          aria-label="Decrease opacity"
        >
          dim
        </button>
        <button
          type="button"
          className="widget-button"
          onClick={() => changeOpacity(0.1)}
          aria-label="Increase opacity"
        >
          brighten
        </button>
        <span className="widget-controls-spacer" />
        <button
          type="button"
          className="widget-button"
          onClick={() => void load()}
          aria-label="Refresh quota"
        >
          refresh
        </button>
        <button
          type="button"
          className="widget-button"
          onClick={() => void showMainWindow()}
          aria-label="Open dashboard"
        >
          dashboard
        </button>
        <button
          type="button"
          className="widget-button"
          onClick={() => void hideWidgetWindow()}
          aria-label="Hide widget"
        >
          hide
        </button>
      </div>

      <main className="widget-body">
        {error ? (
          <p className="widget-empty" role="status">
            quota unavailable
          </p>
        ) : !dashboard ? (
          <p className="widget-empty" role="status">
            loading
          </p>
        ) : !hasAnyProvider ? (
          <p className="widget-empty" role="status">
            no quota data yet
          </p>
        ) : (
          <ul className="widget-providers">
            {dashboard.providers.map((provider) => (
              <ProviderRow key={provider.provider_id} provider={provider} now={now} />
            ))}
          </ul>
        )}
      </main>
    </div>
  );
}
