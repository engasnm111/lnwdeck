import { useCallback, useEffect, useRef, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  fetchQuotaDashboard,
  fetchWidgetSettings,
  hideWidgetWindow,
  refreshAll,
  setWidgetLocked,
  setWidgetOpacity,
  showMainWindow,
  type ProviderQuotaCard,
  type QuotaDashboardData,
  type QuotaWindowData,
  type WidgetSettingsData,
} from "../../lib/native";
import { formatCompact, formatCountdown, formatRefreshedAgo } from "./widgetTime";

const STATUS_LABELS: Record<ProviderQuotaCard["status"], string> = {
  fresh: "OK",
  stale: "stale",
  unavailable: "unavailable",
  auth_expired: "auth expired",
  rate_limited: "rate limited",
  error: "error",
};

/** Credit amounts are carried in micro-credits by the OpenRouter adapter. */
const MICRO_CREDITS = 1_000_000;

function formatAmount(value: number, kind: QuotaWindowData["kind"]): string {
  if (kind === "credits") {
    return (value / MICRO_CREDITS).toFixed(2);
  }
  return formatCompact(value);
}

function barTone(remainingPercent: number): "success" | "warning" | "danger" {
  if (remainingPercent <= 5) {
    return "danger";
  }
  if (remainingPercent <= 20) {
    return "warning";
  }
  return "success";
}

function WindowRow({ window, now }: { window: QuotaWindowData; now: number }) {
  if (window.is_unlimited) {
    return (
      <div className="widget-window">
        <span className="widget-window-metric">Local / Unlimited</span>
      </div>
    );
  }

  const countdown = formatCountdown(window.reset_at, now);

  // Without a real limit there is no bar to draw: the recorded usage is shown
  // and marked as an estimate.
  if (window.remaining_percent === null) {
    return (
      <div className="widget-window">
        <span className="widget-window-metric">
          {window.label}: used {formatAmount(window.used, window.kind)} {window.kind}
          <span className="widget-estimate"> estimate</span>
          {countdown ? ` - resets ${countdown}` : ""}
        </span>
      </div>
    );
  }

  const remainingPercent = window.remaining_percent;
  return (
    <div className="widget-window">
      <div
        className="widget-bar"
        role="progressbar"
        aria-label={`${window.label} remaining`}
        aria-valuenow={Math.round(remainingPercent)}
        aria-valuemin={0}
        aria-valuemax={100}
      >
        <div
          className={`widget-bar-fill widget-bar-fill-${barTone(remainingPercent)}`}
          style={{ width: `${remainingPercent}%` }}
        />
      </div>
      <span className="widget-window-metric">
        {window.label}: {Math.round(remainingPercent)}% left
        {countdown ? ` - resets ${countdown}` : ""}
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
        provider.windows.map((window) => (
          <WindowRow key={window.window_key} window={window} now={now} />
        ))
      )}
    </li>
  );
}

/**
 * Floating quota widget.
 *
 * Opacity, lock mode and visibility come from the backend and are written back
 * through commands, so the widget and the dashboard cannot disagree about them.
 * A window without a real limit never renders a progress bar.
 */
export function FloatingWidget() {
  const [dashboard, setDashboard] = useState<QuotaDashboardData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [now, setNow] = useState(Date.now());
  const [settings, setSettings] = useState<WidgetSettingsData>({
    opacity: 1,
    locked: false,
    visible: true,
  });
  const [refreshing, setRefreshing] = useState(false);
  const unlistenRef = useRef<UnlistenFn[]>([]);

  const load = useCallback(async () => {
    setError(null);
    try {
      setDashboard(await fetchQuotaDashboard());
    } catch (loadError) {
      setError(
        loadError instanceof Error ? loadError.message : "quota unavailable",
      );
    }
  }, []);

  const loadSettings = useCallback(async () => {
    try {
      const result = await fetchWidgetSettings();
      // Only a well-formed payload replaces the current appearance; a missing
      // or malformed one leaves the widget readable instead of blank.
      if (result && typeof result.opacity === "number") {
        setSettings(result);
      }
    } catch {
      // Outside a Tauri runtime the documented defaults apply.
    }
  }, []);

  useEffect(() => {
    void load();
    void loadSettings();

    const dataTick = setInterval(() => {
      setNow(Date.now());
      void load();
    }, 30_000);
    const countdownTick = setInterval(() => setNow(Date.now()), 1_000);

    const subscribe = async () => {
      try {
        unlistenRef.current.push(
          await listen("quota-updated", () => void load()),
        );
        unlistenRef.current.push(
          await listen<WidgetSettingsData>(
            "widget-settings-changed",
            (event) => setSettings(event.payload),
          ),
        );
      } catch {
        // No event bus outside a Tauri runtime; polling still applies.
      }
    };
    void subscribe();

    return () => {
      clearInterval(dataTick);
      clearInterval(countdownTick);
      for (const unlisten of unlistenRef.current) {
        unlisten();
      }
      unlistenRef.current = [];
    };
  }, [load, loadSettings]);

  const changeOpacity = useCallback(async (delta: number) => {
    try {
      const stored = await setWidgetOpacity(
        Math.round((settings.opacity + delta) * 100) / 100,
      );
      setSettings((current) => ({ ...current, opacity: stored }));
    } catch {
      // A refused value leaves the stored opacity untouched.
    }
  }, [settings.opacity]);

  const toggleLock = useCallback(async () => {
    try {
      const stored = await setWidgetLocked(!settings.locked);
      setSettings((current) => ({ ...current, locked: stored }));
    } catch {
      // Keep the previous state when the command fails.
    }
  }, [settings.locked]);

  const handleRefresh = useCallback(async () => {
    setRefreshing(true);
    setError(null);
    try {
      await refreshAll();
      await load();
    } catch (refreshError) {
      setError(
        refreshError instanceof Error
          ? refreshError.message
          : "refresh failed",
      );
    } finally {
      setRefreshing(false);
    }
  }, [load]);

  const hasAnyProvider = (dashboard?.providers.length ?? 0) > 0;

  return (
    <div
      className="widget-root"
      data-tauri-drag-region={settings.locked ? undefined : ""}
      style={{ opacity: settings.opacity }}
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
          onClick={() => void toggleLock()}
          aria-label={settings.locked ? "Unlock widget" : "Lock widget"}
        >
          {settings.locked ? "unlock" : "lock"}
        </button>
        <button
          type="button"
          className="widget-button"
          onClick={() => void changeOpacity(-0.1)}
          aria-label="Decrease opacity"
        >
          dim
        </button>
        <button
          type="button"
          className="widget-button"
          onClick={() => void changeOpacity(0.1)}
          aria-label="Increase opacity"
        >
          brighten
        </button>
        <span className="widget-controls-spacer" />
        <button
          type="button"
          className="widget-button"
          onClick={() => void handleRefresh()}
          disabled={refreshing}
          aria-label="Refresh quota"
        >
          {refreshing ? "..." : "refresh"}
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
          <p className="widget-empty" role="alert">
            {error}
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
              <ProviderRow
                key={provider.provider_id}
                provider={provider}
                now={now}
              />
            ))}
          </ul>
        )}
      </main>
    </div>
  );
}
