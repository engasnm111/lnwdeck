import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  fetchQuotaDashboard,
  fetchWidgetSettings,
  getWidgetPet,
  hideWidgetWindow,
  refreshAll,
  setWidgetLocked,
  setWidgetProviders,
  setWidgetView,
  showMainWindow,
  type ProviderQuotaCard,
  type QuotaDashboardData,
  type QuotaStatus,
  type QuotaWindowData,
  type WidgetSettingsData,
  type WidgetView,
} from "../../lib/native";
import {
  formatCompact,
  formatRefreshedAgo,
  formatRemaining,
  formatResetLabel,
  formatResetShort,
  quotaLevel,
  windowSubtitle,
  type QuotaLevel,
} from "./widgetTime";
import { PetMascot, type ImportedPet } from "./PetMascot";
import { derivePetMood, type PetReaction } from "./petState";
import {
  BarsIcon,
  CalendarIcon,
  ClockIcon,
  DotIcon,
  SparkIcon,
} from "./WidgetIcons";

/** Credits are carried in micro-credits by the OpenRouter adapter. */
const MICRO_CREDITS = 1_000_000;
/** How often the dashboard is re-read. */
const POLL_INTERVAL_MS = 30_000;
/** How often the countdowns tick without refetching. */
const TICK_INTERVAL_MS = 1_000;
/** How long the refresh-success celebration is shown. */
const CELEBRATE_MS = 2_500;

interface StatusChip {
  label: string;
  tone: "ok" | "stale" | "error" | "muted";
  /** Why the provider is in this state, when there is more to say. */
  detail: string | null;
}

/**
 * Maps a provider status to a chip.
 *
 * Each state named in the widget requirements has its own wording, so
 * "not authenticated" is never shown as a generic error and a stale reading is
 * never shown as fresh.
 */
export function statusChip(
  status: QuotaStatus,
  errorCode: string | null,
): StatusChip {
  switch (status) {
    case "fresh":
      return { label: "Live", tone: "ok", detail: null };
    case "stale":
      return {
        label: "Stale",
        tone: "stale",
        detail: "This reading is older than the provider freshness window.",
      };
    case "rate_limited":
      return {
        label: "Rate limited",
        tone: "error",
        detail: "The provider refused further requests for now.",
      };
    case "auth_expired":
      return {
        label: "Not authenticated",
        tone: "error",
        detail: "The stored credential was rejected.",
      };
    case "unavailable":
      return {
        label: "Unavailable",
        tone: "muted",
        detail:
          errorCode === "NOT_CONFIGURED"
            ? "Add an API key in Settings to read this provider."
            : "No source was available for this provider.",
      };
    default:
      return {
        label: "Error",
        tone: "error",
        detail: "The last collection failed.",
      };
  }
}

/**
 * A provider counts as fetched when its quota channel produced a reading.
 * Fresh and stale readings are real data; a failed collection (not
 * configured, not authenticated, rate limited, or an error) hides the
 * provider until it recovers. Mirrors the domain's `QuotaStatus::is_usable`.
 */
export function hasFetchedQuota(status: QuotaStatus): boolean {
  return status === "fresh" || status === "stale";
}

/** Row icon chosen from the window scope and kind. */
function WindowIcon({ window }: { window: QuotaWindowData }) {
  if (window.kind === "credits") {
    return <SparkIcon />;
  }
  switch (window.scope) {
    case "rolling":
    case "session":
      return <ClockIcon />;
    case "weekly":
    case "daily":
      return <BarsIcon />;
    case "monthly":
      return <CalendarIcon />;
    default:
      return <DotIcon />;
  }
}

/** Amount formatting that keeps credits readable. */
function formatUsed(window: QuotaWindowData): string {
  if (window.kind === "credits") {
    return `${(window.used / MICRO_CREDITS).toFixed(2)} credits`;
  }
  return `${formatCompact(window.used)} ${window.kind}`;
}

/** A window with its derived presentation values. */
function windowView(window: QuotaWindowData, now: number) {
  const percent =
    window.remaining_percent === null
      ? null
      : Math.max(0, Math.min(100, window.remaining_percent));
  const level: QuotaLevel | null = percent === null ? null : quotaLevel(percent);
  return {
    percent,
    level,
    resetShort: formatResetShort(window.reset_at, now),
    resetLong: formatResetLabel(window.reset_at, now),
    subtitle: window.is_unlimited
      ? "Local runtime, no quota"
      : windowSubtitle(window.scope, window.kind, window.label),
  };
}

/** One quota window as a labelled row with a full-width bar. */
function BarRow({
  window,
  providerName,
  now,
}: {
  window: QuotaWindowData;
  providerName: string;
  now: number;
}) {
  const view = windowView(window, now);
  const barLabel = `${providerName} ${window.label} remaining`;

  return (
    <div className="w-row">
      <div className="w-row-main">
        <span className={`w-row-icon w-row-icon-${view.level ?? "unknown"}`}>
          <WindowIcon window={window} />
        </span>
        <span className="w-row-text">
          <span className="w-row-title">{window.label}</span>
          <span className="w-row-subtitle">{view.subtitle}</span>
        </span>
        <span className="w-row-value">
          {view.percent === null ? (
            <span className="w-percent w-percent-unknown">
              {formatRemaining(null)}
            </span>
          ) : (
            <span className={`w-percent w-percent-${view.level}`}>
              {Math.round(view.percent)}%
            </span>
          )}
          <span className="w-row-reset">{view.resetLong}</span>
        </span>
      </div>

      {view.percent === null ? (
        <div
          className="w-bar w-bar-unknown"
          role="img"
          aria-label={`${barLabel}: no limit reported, ${formatUsed(window)} used, ${view.resetLong.toLowerCase()}`}
        />
      ) : (
        <div
          className="w-bar"
          role="progressbar"
          aria-label={barLabel}
          aria-valuenow={Math.round(view.percent)}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuetext={`${formatRemaining(view.percent)}, ${view.resetLong.toLowerCase()}`}
        >
          <div
            className={`w-bar-fill w-bar-fill-${view.level}`}
            style={{ width: `${view.percent}%` }}
          />
        </div>
      )}

      {view.percent === null && !window.is_unlimited && (
        <span className="w-row-note">{formatUsed(window)} used, no limit reported</span>
      )}
    </div>
  );
}

/** One quota window as a compact ring gauge. */
function RingGauge({
  window,
  providerName,
  now,
}: {
  window: QuotaWindowData;
  providerName: string;
  now: number;
}) {
  const view = windowView(window, now);
  const radius = 26;
  const circumference = 2 * Math.PI * radius;
  const filled =
    view.percent === null ? 0 : (view.percent / 100) * circumference;
  const barLabel = `${providerName} ${window.label} remaining`;

  return (
    <div className="w-ring">
      <svg
        className="w-ring-svg"
        viewBox="0 0 64 64"
        role={view.percent === null ? "img" : "progressbar"}
        aria-label={
          view.percent === null
            ? `${barLabel}: no limit reported`
            : barLabel
        }
        aria-valuenow={view.percent === null ? undefined : Math.round(view.percent)}
        aria-valuemin={view.percent === null ? undefined : 0}
        aria-valuemax={view.percent === null ? undefined : 100}
        aria-valuetext={
          view.percent === null
            ? undefined
            : `${formatRemaining(view.percent)}, ${view.resetLong.toLowerCase()}`
        }
      >
        <circle className="w-ring-track" cx="32" cy="32" r={radius} />
        {view.percent !== null && (
          <circle
            className={`w-ring-fill w-ring-fill-${view.level}`}
            cx="32"
            cy="32"
            r={radius}
            strokeDasharray={`${filled} ${circumference - filled}`}
            strokeDashoffset={circumference * 0.25}
          />
        )}
      </svg>
      <span className="w-ring-value">
        {view.percent === null ? "--" : `${Math.round(view.percent)}%`}
      </span>
      <span className="w-ring-label">{window.label}</span>
      <span className="w-ring-reset">{view.resetShort}</span>
    </div>
  );
}

function ProviderCard({
  provider,
  view,
  now,
}: {
  provider: ProviderQuotaCard;
  view: WidgetView;
  now: number;
}) {
  const chip = statusChip(provider.status, provider.error_code);
  return (
    <li className="w-card">
      <div className="w-card-head">
        <span className="w-card-name">{provider.display_name}</span>
        <span className={`w-chip w-chip-${chip.tone}`}>{chip.label}</span>
      </div>

      {provider.windows.length === 0 ? (
        <p className="w-card-note">{chip.detail ?? "No quota was reported."}</p>
      ) : view === "rings" ? (
        <div className="w-rings">
          {provider.windows.map((window) => (
            <RingGauge
              key={window.window_key}
              window={window}
              providerName={provider.display_name}
              now={now}
            />
          ))}
        </div>
      ) : (
        provider.windows.map((window) => (
          <BarRow
            key={window.window_key}
            window={window}
            providerName={provider.display_name}
            now={now}
          />
        ))
      )}

      {provider.error_code && (
        <p className="w-card-code">{provider.error_code}</p>
      )}
    </li>
  );
}

/**
 * Floating quota widget.
 *
 * A dedicated always-on-top window with its own HTML entry: it does not load the
 * dashboard shell. Only providers that actually reported data appear, a bar or a
 * ring is drawn only where the provider published a real limit, and appearance,
 * lock state, layout and the provider selection are held by the backend so the
 * window and the dashboard cannot disagree.
 */
export function FloatingWidget() {
  const [dashboard, setDashboard] = useState<QuotaDashboardData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [now, setNow] = useState(() => Date.now());
  const [settings, setSettings] = useState<WidgetSettingsData>({
    opacity: 1,
    locked: false,
    visible: true,
    selected_providers: [],
    view: "bars",
    pet_id: "",
  });
  const [refreshing, setRefreshing] = useState(false);
  const [pickerOpen, setPickerOpen] = useState(false);
  const [reaction, setReaction] = useState<PetReaction>(null);
  const reactionTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [activePet, setActivePet] = useState<ImportedPet | null>(null);
  const unlistenRef = useRef<UnlistenFn[]>([]);

  const load = useCallback(async () => {
    try {
      const result = await fetchQuotaDashboard();
      setDashboard(result);
      setError(null);
    } catch (loadError) {
      setError(
        loadError instanceof Error ? loadError.message : "quota unavailable",
      );
    } finally {
      setLoading(false);
    }
  }, []);

  const applySettings = useCallback((payload: WidgetSettingsData) => {
    setSettings({
      ...payload,
      selected_providers: payload.selected_providers ?? [],
      pet_id: payload.pet_id ?? "",
      view:
        payload.view === "rings" || payload.view === "pet" ? payload.view : "bars",
    });
  }, []);

  const loadSettings = useCallback(async () => {
    try {
      const result = await fetchWidgetSettings();
      if (result && typeof result.opacity === "number") {
        applySettings(result);
      }
    } catch {
      // Outside a Tauri runtime the documented defaults apply.
    }
  }, [applySettings]);

  useEffect(() => {
    void load();
    void loadSettings();

    const poll = setInterval(() => {
      setNow(Date.now());
      void load();
    }, POLL_INTERVAL_MS);
    const tick = setInterval(() => setNow(Date.now()), TICK_INTERVAL_MS);

    const subscribe = async () => {
      try {
        unlistenRef.current.push(
          await listen("quota-updated", () => void load()),
        );
        unlistenRef.current.push(
          await listen<WidgetSettingsData>(
            "widget-settings-changed",
            (event) => applySettings(event.payload),
          ),
        );
      } catch {
        // No event bus outside a Tauri runtime; polling still applies.
      }
    };
    void subscribe();

    return () => {
      clearInterval(poll);
      clearInterval(tick);
      for (const unlisten of unlistenRef.current) {
        unlisten();
      }
      unlistenRef.current = [];
    };
  }, [load, loadSettings, applySettings]);

  /**
   * Starts the brief refresh-success celebration. A repeated refresh replaces
   * the pending timer instead of stacking timers, so rapid clicks cannot race.
   */
  const celebrate = useCallback(() => {
    if (reactionTimer.current !== null) {
      clearTimeout(reactionTimer.current);
    }
    reactionTimer.current = setTimeout(() => {
      reactionTimer.current = null;
      setReaction(null);
    }, CELEBRATE_MS);
    setReaction("celebrate");
  }, []);

  // The celebration timer must not outlive the widget.
  useEffect(() => {
    return () => {
      if (reactionTimer.current !== null) {
        clearTimeout(reactionTimer.current);
        reactionTimer.current = null;
      }
    };
  }, []);

  // Loads the selected community pet's manifest. A missing or invalid pet
  // silently falls back to the built-in robot.
  useEffect(() => {
    let cancelled = false;
    if (!settings.pet_id) {
      setActivePet(null);
      return undefined;
    }
    void getWidgetPet()
      .then((pet) => {
        if (cancelled) {
          return;
        }
        setActivePet(
          pet
            ? {
                id: pet.id,
                displayName: pet.displayName,
                spriteVersionNumber: pet.spriteVersionNumber,
              }
            : null,
        );
      })
      .catch(() => {
        if (!cancelled) {
          setActivePet(null);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [settings.pet_id]);

  const handleRefresh = useCallback(async () => {
    setRefreshing(true);
    setError(null);
    try {
      await refreshAll();
      await load();
      setNow(Date.now());
      celebrate();
    } catch (refreshError) {
      setError(
        refreshError instanceof Error ? refreshError.message : "refresh failed",
      );
    } finally {
      setRefreshing(false);
    }
  }, [load, celebrate]);

  const toggleLock = useCallback(async () => {
    try {
      const stored = await setWidgetLocked(!settings.locked);
      setSettings((current) => ({ ...current, locked: stored }));
    } catch {
      // A failed write leaves the previous state visible.
    }
  }, [settings.locked]);

  const selectView = useCallback(async (view: WidgetView) => {
    if (view !== "bars" && view !== "rings" && view !== "pet") {
      return;
    }
    try {
      const stored = await setWidgetView(view);
      const safe: WidgetView =
        stored === "rings" || stored === "pet" ? stored : "bars";
      setSettings((current) => ({ ...current, view: safe }));
    } catch {
      // Keep the current layout when the write is refused.
    }
  }, []);

  const toggleProvider = useCallback(
    async (providerId: string) => {
      const current = settings.selected_providers;
      const next = current.includes(providerId)
        ? current.filter((id) => id !== providerId)
        : [...current, providerId];
      try {
        const stored = await setWidgetProviders(next);
        setSettings((state) => ({ ...state, selected_providers: stored }));
      } catch {
        // Keep the current selection when the write is refused.
      }
    },
    [settings.selected_providers],
  );

  // Escape hides the widget, matching the close button.
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        void hideWidgetWindow();
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, []);

  const allProviders = dashboard?.providers ?? [];
  // Providers whose quota collection actually produced data. Failed
  // collections are not shown in the widget; the dashboard explains them.
  const fetchedProviders = useMemo(
    () => allProviders.filter((provider) => hasFetchedQuota(provider.status)),
    [allProviders],
  );
  const visibleProviders = useMemo(() => {
    if (settings.selected_providers.length === 0) {
      return fetchedProviders;
    }
    return fetchedProviders.filter((provider) =>
      settings.selected_providers.includes(provider.provider_id),
    );
  }, [fetchedProviders, settings.selected_providers]);

  // The pet mood derives only from what the widget currently shows, after the
  // fetch filter and provider selection above have been applied.
  const petMood = useMemo(() => derivePetMood(visibleProviders), [visibleProviders]);

  const dragProps = settings.locked ? {} : { "data-tauri-drag-region": "" };

  return (
    <div
      className="w-root"
      data-locked={settings.locked ? "true" : "false"}
      data-view={settings.view}
      style={{ opacity: settings.opacity }}
    >
      <header className="w-header" {...dragProps}>
        <span className="w-brand" {...dragProps}>
          lnwdeck
        </span>
        <div className="w-header-actions">
          <button
            type="button"
            className="w-btn"
            onClick={() => void handleRefresh()}
            disabled={refreshing}
            aria-label="Refresh quota"
            title="Refresh quota"
          >
            {refreshing ? "..." : "Sync"}
          </button>
          <button
            type="button"
            className="w-btn"
            onClick={() => void showMainWindow()}
            aria-label="Open dashboard"
            title="Open the dashboard window"
          >
            Open
          </button>
          <select
            className="w-btn w-view-select"
            value={settings.view}
            onChange={(event) =>
              void selectView(event.target.value as WidgetView)
            }
            aria-label="Widget layout"
            title="Choose the widget layout"
          >
            <option value="bars">Bars</option>
            <option value="rings">Rings</option>
            <option value="pet">Pet</option>
          </select>
          <button
            type="button"
            className={`w-btn ${settings.locked ? "w-btn-active" : ""}`.trim()}
            onClick={() => void toggleLock()}
            aria-label={settings.locked ? "Unlock widget" : "Lock widget"}
            aria-pressed={settings.locked}
            title={
              settings.locked
                ? "Unlock so the widget can be dragged"
                : "Lock the widget in place"
            }
          >
            {settings.locked ? "Lock on" : "Lock off"}
          </button>
          <button
            type="button"
            className={`w-btn ${pickerOpen ? "w-btn-active" : ""}`.trim()}
            onClick={() => setPickerOpen((open) => !open)}
            aria-label="Choose providers"
            aria-expanded={pickerOpen}
            title="Choose which providers the widget shows"
          >
            Filter
          </button>
          <button
            type="button"
            className="w-btn w-btn-danger"
            onClick={() => void hideWidgetWindow()}
            aria-label="Close widget"
            title="Close the widget"
          >
            Close
          </button>
        </div>
      </header>

      {pickerOpen && (
        <div className="w-picker">
          <span className="w-picker-title" id="w-picker-title">
            Providers shown
          </span>
          {allProviders.length === 0 ? (
            <p className="w-message-detail">
              No provider has reported data yet.
            </p>
          ) : (
            <div
              className="w-picker-list"
              role="group"
              aria-labelledby="w-picker-title"
            >
              {allProviders.map((provider) => {
                const pinned =
                  settings.selected_providers.length === 0 ||
                  settings.selected_providers.includes(provider.provider_id);
                return (
                  <button
                    key={provider.provider_id}
                    type="button"
                    className="w-tag"
                    aria-pressed={pinned}
                    onClick={() => void toggleProvider(provider.provider_id)}
                  >
                    {provider.display_name}
                  </button>
                );
              })}
            </div>
          )}
        </div>
      )}

      {settings.view === "pet" && (
        <PetMascot
          mood={petMood}
          reaction={reaction}
          locked={settings.locked}
          imported={activePet}
        />
      )}

      <main className="w-body">
        {error ? (
          <div className="w-message w-message-error" role="alert">
            <span className="w-message-title">Quota unavailable</span>
            <span className="w-message-detail">{error}</span>
          </div>
        ) : loading ? (
          <div className="w-message" role="status" aria-live="polite">
            <span className="w-message-title">Loading</span>
            <span className="w-message-detail">Reading stored quota</span>
          </div>
        ) : allProviders.length === 0 ? (
          <div className="w-message" role="status">
            <span className="w-message-title">No quota data yet</span>
            <span className="w-message-detail">
              Refresh, or open the dashboard to see which collectors found a
              source.
            </span>
          </div>
        ) : fetchedProviders.length === 0 ? (
          <div className="w-message" role="status">
            <span className="w-message-title">No quota data available</span>
            <span className="w-message-detail">
              Every provider failed to fetch quota. Open the dashboard to see
              why.
            </span>
          </div>
        ) : visibleProviders.length === 0 ? (
          <div className="w-message" role="status">
            <span className="w-message-title">No provider selected</span>
            <span className="w-message-detail">
              Every reporting provider is hidden by the current selection.
            </span>
          </div>
        ) : (
          <ul className="w-cards" aria-label="Provider quota">
            {visibleProviders.map((provider) => (
              <ProviderCard
                key={provider.provider_id}
                provider={provider}
                view={settings.view === "rings" ? "rings" : "bars"}
                now={now}
              />
            ))}
          </ul>
        )}
      </main>

      <footer className="w-footer">
        <span className="w-footer-updated">
          Updated {dashboard ? formatRefreshedAgo(dashboard.generated_at, now) : "never"}
        </span>
        <span className="w-footer-spacer" />
        {fetchedProviders.length > 0 && (
          <span className="w-footer-count">
            {visibleProviders.length} of {fetchedProviders.length} providers
          </span>
        )}
        <span className="w-footer-interval">
          <span className="w-dot" aria-hidden="true" />
          {POLL_INTERVAL_MS / 1000}s
        </span>
      </footer>
    </div>
  );
}
