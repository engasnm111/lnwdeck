import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  fetchQuotaDashboard,
  fetchWidgetSettings,
  getWidgetPet,
  hideWidgetWindow,
  startRefresh,
  setWidgetLocked,
  setWidgetProviders,
  setWidgetView,
  showMainWindow,
  type ProviderQuotaCard,
  type QuotaDashboardData,
  type QuotaStatus,
  type QuotaWindowData,
  type RefreshProgressEvent,
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
  type WidgetTranslator,
} from "./widgetTime";
import { PetMascot, type ImportedPet } from "./PetMascot";
import { derivePetMood, type PetReaction } from "./petState";
import { translate, useI18n } from "../../lib/i18n";
import { providerDisplayName } from "../../components/ProviderLogo";
import {
  BarsIcon,
  CalendarIcon,
  CloseIcon,
  ClockIcon,
  DotIcon,
  ExternalLinkIcon,
  FilterIcon,
  LockIcon,
  RefreshIcon,
  SparkIcon,
  WidgetLayoutIcon,
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
  t: (key: string, vars?: Record<string, string>) => string = (key, vars) =>
    translate("en", key, vars),
): StatusChip {
  switch (status) {
    case "fresh":
      return { label: t("widget.status.live"), tone: "ok", detail: null };
    case "stale":
      return {
        label: t("widget.status.stale"),
        tone: "stale",
        detail: t("widget.status.staleDetail"),
      };
    case "rate_limited":
      return {
        label: t("widget.status.rateLimited"),
        tone: "error",
        detail: t("widget.status.rateLimitedDetail"),
      };
    case "auth_expired":
      return {
        label: t("widget.status.notAuthenticated"),
        tone: "error",
        detail: t("widget.status.notAuthenticatedDetail"),
      };
    case "unavailable":
      return {
        label: t("widget.status.unavailable"),
        tone: "muted",
        detail:
          errorCode === "NOT_CONFIGURED"
            ? t("widget.status.notConfigured")
            : t("widget.status.noSource"),
      };
    default:
      return {
        label: t("widget.status.error"),
        tone: "error",
        detail: t("widget.status.errorDetail"),
      };
  }
}

/**
 * A provider counts as fetched when its quota channel produced a reading.
 * Fresh and stale readings are real data; a failed collection (not
 * configured, not authenticated, rate limited, or an error) hides the
 * provider until it recovers. Mirrors the domain's `QuotaStatus::is_usable`.
 */
export function hasFetchedQuota(
  status: QuotaStatus,
  connectionState: ProviderQuotaCard["connection_state"] = "connected",
  quotaSupport: ProviderQuotaCard["quota_support"] = "supported",
  source = "provider_api",
): boolean {
  return (
    (status === "fresh" || status === "stale") &&
    connectionState === "connected" &&
    quotaSupport === "supported" &&
    source !== "local_estimate"
  );
}

/** Missing local integrations are expected on a multi-machine installation. */
export function isNonActionableRefreshError(errorCode: string | null): boolean {
  return (
    errorCode === "SOURCE_UNAVAILABLE" ||
    errorCode === "NOT_INSTALLED" ||
    errorCode === "NOT_CONFIGURED" ||
    errorCode === "NOT_SUPPORTED" ||
    errorCode === "UNSUPPORTED"
  );
}

function providerStatusChip(
  provider: ProviderQuotaCard,
  t: (key: string, vars?: Record<string, string>) => string,
): StatusChip {
  if (provider.connection_state === "not_detected") {
    return {
      label: t("widget.status.noConnection"),
      tone: "muted",
      detail: t("widget.status.noConnectionDetail"),
    };
  }
  if (provider.connection_state === "unsupported") {
    return {
      label: t("widget.status.unavailable"),
      tone: "muted",
      detail: t("widget.status.notSupported"),
    };
  }
  if (
    provider.quota_support === "local estimate" ||
    provider.source === "local_estimate"
  ) {
    return {
      label: t("widget.status.usageOnly"),
      tone: "muted",
      detail: t("widget.status.usageOnlyDetail"),
    };
  }
  return statusChip(provider.status, provider.error_code, t);
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
function formatUsed(
  window: QuotaWindowData,
  t: WidgetTranslator,
): string {
  if (window.kind === "credits") {
    return t("widget.used", {
      value: (window.used / MICRO_CREDITS).toFixed(2),
      unit: t("widget.time.unitCredits"),
    });
  }
  const unitKey =
    window.kind === "tokens"
      ? "widget.time.unitTokens"
      : window.kind === "parallel"
        ? "widget.time.unitParallel"
        : "widget.time.unitRequests";
  return t("widget.used", {
    value: formatCompact(window.used),
    unit: t(unitKey),
  });
}

/** A window with its derived presentation values. */
function windowView(
  window: QuotaWindowData,
  now: number,
  t: WidgetTranslator,
  locale: string,
) {
  const percent =
    window.remaining_percent === null
      ? null
      : Math.max(0, Math.min(100, window.remaining_percent));
  const level: QuotaLevel | null = percent === null ? null : quotaLevel(percent);
  return {
    percent,
    level,
    resetShort: formatResetShort(window.reset_at, now, t, locale),
    resetLong: formatResetLabel(window.reset_at, now, t),
    subtitle: window.is_unlimited
      ? t("widget.localRuntime")
      : windowSubtitle(window.scope, window.kind, window.label, t),
  };
}

/** One quota window as a labelled row with a full-width bar. */
function BarRow({
  window,
  providerName,
  now,
  t,
  locale,
}: {
  window: QuotaWindowData;
  providerName: string;
  now: number;
  t: WidgetTranslator;
  locale: string;
}) {
  const view = windowView(window, now, t, locale);
  const barLabel = t("widget.remainingAria", {
    provider: providerName,
    label: window.label,
  });

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
              {formatRemaining(null, t)}
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
          aria-label={t("widget.noLimitAria", {
            provider: providerName,
            label: window.label,
            used: formatUsed(window, t),
            reset: view.resetLong,
          })}
        />
      ) : (
        <div
          className="w-bar"
          role="progressbar"
          aria-label={barLabel}
          aria-valuenow={Math.round(view.percent)}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuetext={`${formatRemaining(view.percent, t)}, ${view.resetLong}`}
        >
          <div
            className={`w-bar-fill w-bar-fill-${view.level}`}
            style={{ width: `${view.percent}%` }}
          />
        </div>
      )}

      {view.percent === null && !window.is_unlimited && (
        <span className="w-row-note">
          {t("widget.noLimitReported", { used: formatUsed(window, t) })}
        </span>
      )}
    </div>
  );
}

/** One quota window as a compact ring gauge. */
function RingGauge({
  window,
  providerName,
  now,
  t,
  locale,
}: {
  window: QuotaWindowData;
  providerName: string;
  now: number;
  t: WidgetTranslator;
  locale: string;
}) {
  const view = windowView(window, now, t, locale);
  const radius = 26;
  const circumference = 2 * Math.PI * radius;
  const filled =
    view.percent === null ? 0 : (view.percent / 100) * circumference;
  const barLabel = t("widget.remainingAria", {
    provider: providerName,
    label: window.label,
  });

  return (
    <div className="w-ring">
      <svg
        className="w-ring-svg"
        viewBox="0 0 64 64"
        role={view.percent === null ? "img" : "progressbar"}
        aria-label={
          view.percent === null
            ? t("widget.noLimitAria", {
                provider: providerName,
                label: window.label,
                used: formatUsed(window, t),
                reset: view.resetLong,
              })
            : barLabel
        }
        aria-valuenow={view.percent === null ? undefined : Math.round(view.percent)}
        aria-valuemin={view.percent === null ? undefined : 0}
        aria-valuemax={view.percent === null ? undefined : 100}
        aria-valuetext={
          view.percent === null
            ? undefined
            : `${formatRemaining(view.percent, t)}, ${view.resetLong}`
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
  const { language, t } = useI18n();
  const chip = providerStatusChip(provider, t);
  const hasAuthoritativeQuota =
    provider.quota_support === "supported" &&
    provider.source !== "local_estimate";
  const displayName = providerDisplayName({
    provider_id: provider.provider_id,
    display_name: provider.display_name,
  });
  const accountLabel = provider.account_index == null
    ? null
    : t("providers.account", { number: String(provider.account_index) });
  const labeledDisplayName = accountLabel
    ? `${displayName} - ${accountLabel}`
    : displayName;
  const visibleErrorCode = isNonActionableRefreshError(provider.error_code)
    ? null
    : provider.error_code;
  return (
    <li className="w-card">
      <div className="w-card-head">
        <span className="w-card-name" title={labeledDisplayName} aria-label={labeledDisplayName}>
          {labeledDisplayName}
        </span>
        <span className={`w-chip w-chip-${chip.tone}`}>{chip.label}</span>
      </div>

      {!hasAuthoritativeQuota || provider.windows.length === 0 ? (
        <p className="w-card-note">{chip.detail ?? t("widget.noQuotaAvailable")}</p>
      ) : view === "rings" ? (
        <div className="w-rings">
          {provider.windows.map((window) => (
            <RingGauge
              key={`${provider.account_index ?? "default"}-${window.window_key}`}
              window={window}
              providerName={labeledDisplayName}
              now={now}
              t={t}
              locale={language}
            />
          ))}
        </div>
      ) : (
        provider.windows.map((window) => (
          <BarRow
            key={`${provider.account_index ?? "default"}-${window.window_key}`}
            window={window}
            providerName={labeledDisplayName}
            now={now}
            t={t}
            locale={language}
          />
        ))
      )}

      {visibleErrorCode && (
        <p className="w-card-code">{visibleErrorCode}</p>
      )}
    </li>
  );
}

interface OverlayPosition {
  top: number;
  left: number;
}

function overlayPosition(
  anchor: HTMLElement | null,
  width: number,
  height: number,
): OverlayPosition {
  if (!anchor || typeof window === "undefined") return { top: 8, left: 8 };
  const rect = anchor.getBoundingClientRect();
  const maxLeft = Math.max(8, window.innerWidth - width - 8);
  const maxTop = Math.max(8, window.innerHeight - height - 8);
  return {
    top: Math.min(rect.bottom + 4, maxTop),
    left: Math.min(Math.max(8, rect.right - width), maxLeft),
  };
}

function WidgetViewMenu({
  value,
  onChange,
  t,
}: {
  value: WidgetView;
  onChange: (view: WidgetView) => void;
  t: (key: string, vars?: Record<string, string>) => string;
}) {
  const [open, setOpen] = useState(false);
  const [position, setPosition] = useState<OverlayPosition>({ top: 8, left: 8 });
  const rootRef = useRef<HTMLDivElement | null>(null);
  const triggerRef = useRef<HTMLButtonElement | null>(null);
  const optionRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const options: Array<{ value: WidgetView; label: string }> = [
    { value: "bars", label: t("widget.viewBars") },
    { value: "rings", label: t("widget.viewRings") },
    { value: "pet", label: t("widget.viewPet") },
  ];
  const currentIndex = Math.max(
    0,
    options.findIndex((option) => option.value === value),
  );

  useEffect(() => {
    if (!open) return undefined;
    const updatePosition = () => {
      setPosition(overlayPosition(triggerRef.current, 128, 150));
    };
    const onPointerDown = (event: PointerEvent) => {
      const target = event.target instanceof Element ? event.target : null;
      const insidePortal = target?.closest("[data-widget-overlay]") !== null;
      if (!rootRef.current?.contains(event.target as Node) && !insidePortal) {
        setOpen(false);
      }
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setOpen(false);
        triggerRef.current?.focus();
      }
    };
    updatePosition();
    window.addEventListener("resize", updatePosition);
    window.addEventListener("scroll", updatePosition, true);
    document.addEventListener("pointerdown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("resize", updatePosition);
      window.removeEventListener("scroll", updatePosition, true);
      document.removeEventListener("pointerdown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [open]);

  useEffect(() => {
    if (open) optionRefs.current[currentIndex]?.focus();
  }, [currentIndex, open]);

  const move = (offset: number) => {
    const next = (currentIndex + offset + options.length) % options.length;
    onChange(options[next].value);
    optionRefs.current[next]?.focus();
  };

  const optionsSurface = open && typeof document !== "undefined"
    ? createPortal(
        <div
          className="w-view-options"
          data-widget-overlay="true"
          role="listbox"
          aria-label={t("widget.action.layout")}
          style={{ top: position.top, left: position.left }}
        >
          {options.map((option, index) => (
            <button
              type="button"
              key={option.value}
              ref={(node) => {
                optionRefs.current[index] = node;
              }}
              className="w-view-option"
              role="option"
              aria-selected={value === option.value}
              onClick={() => {
                onChange(option.value);
                setOpen(false);
                triggerRef.current?.focus();
              }}
              onKeyDown={(event) => {
                if (event.key === "ArrowDown") {
                  event.preventDefault();
                  move(1);
                } else if (event.key === "ArrowUp") {
                  event.preventDefault();
                  move(-1);
                } else if (event.key === "Escape") {
                  event.preventDefault();
                  setOpen(false);
                  triggerRef.current?.focus();
                } else if (event.key === "Enter" || event.key === " ") {
                  event.preventDefault();
                  onChange(option.value);
                  setOpen(false);
                  triggerRef.current?.focus();
                }
              }}
            >
              {option.label}
            </button>
          ))}
        </div>,
        document.body,
      )
    : null;

  return (
    <div className="w-view-menu" ref={rootRef}>
      <button
        type="button"
        ref={triggerRef}
        className="w-btn w-view-trigger"
        data-widget-icon-action="true"
        aria-label={t("widget.action.layout")}
        title={t("widget.action.layoutTitle")}
        aria-haspopup="listbox"
        aria-expanded={open}
        onClick={() => setOpen((current) => !current)}
        onKeyDown={(event) => {
          if (event.key === "ArrowDown" || event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            setOpen(true);
          } else if (event.key === "ArrowUp") {
            event.preventDefault();
            setOpen(true);
          }
        }}
      >
        <WidgetLayoutIcon view={options[currentIndex].value} />
      </button>
      {optionsSurface}
    </div>
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
  const { t } = useI18n();
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
    size_preset: "medium",
  });
  const [refreshing, setRefreshing] = useState(false);
  const [pickerOpen, setPickerOpen] = useState(false);
  const [pickerPosition, setPickerPosition] = useState<OverlayPosition>({ top: 8, left: 8 });
  const pickerButtonRef = useRef<HTMLButtonElement | null>(null);
  const [reaction, setReaction] = useState<PetReaction>(null);
  const reactionTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [activePet, setActivePet] = useState<ImportedPet | null>(null);
  const unlistenRef = useRef<UnlistenFn[]>([]);

  useEffect(() => {
    if (!pickerOpen) return undefined;
    const updatePosition = () => {
      setPickerPosition(overlayPosition(pickerButtonRef.current, 276, 320));
    };
    const onPointerDown = (event: PointerEvent) => {
      const target = event.target instanceof Element ? event.target : null;
      const inPicker = target?.closest(".w-picker") !== null;
      if (!pickerButtonRef.current?.contains(event.target as Node) && !inPicker) {
        setPickerOpen(false);
      }
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setPickerOpen(false);
        pickerButtonRef.current?.focus();
      }
    };
    updatePosition();
    window.addEventListener("resize", updatePosition);
    window.addEventListener("scroll", updatePosition, true);
    document.addEventListener("pointerdown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("resize", updatePosition);
      window.removeEventListener("scroll", updatePosition, true);
      document.removeEventListener("pointerdown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [pickerOpen]);

  const load = useCallback(async () => {
    try {
      const result = await fetchQuotaDashboard();
      setDashboard(result);
      setError(null);
    } catch (loadError) {
      setError(
        loadError instanceof Error ? loadError.message : t("widget.error.quota"),
      );
    } finally {
      setLoading(false);
    }
  }, [t]);

  const applySettings = useCallback((payload: WidgetSettingsData) => {
    setSettings({
      ...payload,
      selected_providers: payload.selected_providers ?? [],
      pet_id: payload.pet_id ?? "",
      view:
        payload.view === "rings" || payload.view === "pet" ? payload.view : "bars",
      size_preset:
        payload.size_preset === "small" ||
        payload.size_preset === "medium" ||
        payload.size_preset === "large"
          ? payload.size_preset
          : "medium",
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

  // The widget observes the same background job as the dashboard and tray.
  // It never waits on provider I/O; only the small event payload crosses IPC.
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    let cancelled = false;
    void listen<RefreshProgressEvent>("refresh-progress", (event) => {
      const progress = event.payload;
      if (progress.phase === "started" || progress.phase === "progress") {
        setRefreshing(true);
        return;
      }
      setRefreshing(false);
      if (
        progress.phase === "failed" ||
        (progress.phase === "partial" &&
          !isNonActionableRefreshError(progress.error_code))
      ) {
        setError(t("widget.error.refresh"));
        void load();
        return;
      }
      void load();
      setNow(Date.now());
      celebrate();
    }).then((cleanup) => {
      if (cancelled) {
        cleanup();
      } else {
        unlisten = cleanup;
      }
    }).catch(() => {
      // Outside Tauri, the polling fallback remains active.
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [celebrate, load, t]);

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
      const result = await startRefresh();
      if (!result.started && !result.already_running) {
        setRefreshing(false);
      }
    } catch (refreshError) {
      setError(
        refreshError instanceof Error
          ? refreshError.message
          : t("widget.error.refresh"),
      );
      setRefreshing(false);
    }
  }, [t]);

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
    () =>
      allProviders.filter((provider) =>
        hasFetchedQuota(
          provider.status,
          provider.connection_state,
          provider.quota_support,
          provider.source,
        ),
      ),
    [allProviders],
  );
  const visibleProviders = useMemo(() => {
    if (settings.selected_providers.length === 0) {
      return fetchedProviders;
    }
    return allProviders.filter((provider) =>
      settings.selected_providers.includes(provider.provider_id),
    );
  }, [allProviders, fetchedProviders, settings.selected_providers]);
  const hasExplicitSelection = settings.selected_providers.length > 0;

  // The pet mood derives only from what the widget currently shows, after the
  // the fetch filter and provider selection above have been applied. A pinned
  // disconnected/usage-only card must not make the pet look quota-starved.
  const petMoodProviders = useMemo(
    () =>
      visibleProviders.filter((provider) =>
        hasFetchedQuota(
          provider.status,
          provider.connection_state,
          provider.quota_support,
          provider.source,
        ),
      ),
    [visibleProviders],
  );
  const petMood = useMemo(
    () => derivePetMood(petMoodProviders),
    [petMoodProviders],
  );

  const dragProps = settings.locked ? {} : { "data-tauri-drag-region": "" };

  return (
    <div
      className="w-root w-root-single-surface"
      data-locked={settings.locked ? "true" : "false"}
      data-view={settings.view}
      style={{ opacity: settings.opacity }}
    >
      <header className="w-header">
        <span className="w-brand" {...dragProps}>
          lnwdeck
        </span>
        <span
          className="w-header-drag-space"
          aria-hidden="true"
          {...dragProps}
        />
        <div className="w-header-actions">
          <button
            type="button"
            className="w-btn"
            data-widget-icon-action="true"
            onClick={() => void handleRefresh()}
            disabled={refreshing}
            aria-label={t("widget.action.refreshQuota")}
            title={t("widget.action.refreshQuota")}
          >
            <span className={refreshing ? "w-icon-spin" : ""}>
              <RefreshIcon />
            </span>
          </button>
          <button
            type="button"
            className="w-btn"
            data-widget-icon-action="true"
            onClick={() => void showMainWindow()}
            aria-label={t("widget.action.open")}
            title={t("widget.action.openTitle")}
          >
            <ExternalLinkIcon />
          </button>
          <WidgetViewMenu value={settings.view} onChange={(view) => void selectView(view)} t={t} />
          <button
            type="button"
            className={`w-btn ${settings.locked ? "w-btn-active" : ""}`.trim()}
            data-widget-icon-action="true"
            onClick={() => void toggleLock()}
            aria-label={settings.locked ? t("widget.action.unlock") : t("widget.action.lock")}
            aria-pressed={settings.locked}
            title={
              settings.locked
                ? t("widget.action.unlockTitle")
                : t("widget.action.lockTitle")
            }
          >
            <LockIcon locked={settings.locked} />
          </button>
          <button
            type="button"
            ref={pickerButtonRef}
            className={`w-btn w-picker-trigger ${pickerOpen ? "w-btn-active" : ""}`.trim()}
            data-widget-icon-action="true"
            data-widget-picker-trigger="true"
            onClick={() => setPickerOpen((open) => !open)}
            aria-label={t("widget.action.providers")}
            aria-expanded={pickerOpen}
            aria-controls="w-provider-picker"
            title={t("widget.action.providersTitle")}
          >
            <FilterIcon />
          </button>
          <button
            type="button"
            className="w-btn w-btn-danger"
            data-widget-icon-action="true"
            onClick={() => void hideWidgetWindow()}
            aria-label={t("widget.action.close")}
            title={t("widget.action.closeTitle")}
          >
            <CloseIcon />
          </button>
        </div>
      </header>

      {pickerOpen && typeof document !== "undefined" && createPortal(
        <div
          className="w-picker"
          id="w-provider-picker"
          data-widget-overlay="true"
          data-surface="opaque"
          role="dialog"
          aria-labelledby="w-picker-title"
          style={{ top: pickerPosition.top, left: pickerPosition.left }}
        >
          <span className="w-picker-title" id="w-picker-title">
            {t("widget.providersShown")}
          </span>
          {allProviders.length === 0 ? (
            <p className="w-message-detail">
              {t("widget.noProviderReported")}
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
                const displayName = providerDisplayName({
                  provider_id: provider.provider_id,
                  display_name: provider.display_name,
                });
                const accountLabel = provider.account_index == null
                  ? null
                  : t("providers.account", { number: String(provider.account_index) });
                const labeledDisplayName = accountLabel
                  ? `${displayName} - ${accountLabel}`
                  : displayName;
                return (
                  <button
                    key={`${provider.provider_id}-${provider.account_index ?? "default"}`}
                    type="button"
                    className="w-tag"
                    title={labeledDisplayName}
                    aria-pressed={pinned}
                    onClick={() => void toggleProvider(provider.provider_id)}
                  >
                    {labeledDisplayName}
                  </button>
                );
              })}
            </div>
          )}
        </div>,
        document.body,
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
            <span className="w-message-title">{t("widget.quotaUnavailable")}</span>
            <span className="w-message-detail">{error}</span>
          </div>
        ) : loading ? (
          <div className="w-message" role="status" aria-live="polite">
            <span className="w-message-title">{t("widget.loading")}</span>
            <span className="w-message-detail">{t("widget.loadingDetail")}</span>
          </div>
        ) : allProviders.length === 0 ? (
          <div className="w-message" role="status">
            <span className="w-message-title">{t("widget.noQuotaYet")}</span>
            <span className="w-message-detail">
              {t("widget.noQuotaYetDetail")}
            </span>
          </div>
        ) : !hasExplicitSelection && fetchedProviders.length === 0 ? (
          <div className="w-message" role="status">
            <span className="w-message-title">{t("widget.noQuotaAvailable")}</span>
            <span className="w-message-detail">
              {t("widget.noQuotaAvailableDetail")}
            </span>
          </div>
        ) : visibleProviders.length === 0 ? (
          <div className="w-message" role="status">
            <span className="w-message-title">{t("widget.noProviderSelected")}</span>
            <span className="w-message-detail">
              {t("widget.noProviderSelectedDetail")}
            </span>
          </div>
        ) : (
          <ul className="w-cards" aria-label={t("widget.providerQuota")}>
            {visibleProviders.map((provider) => (
              <ProviderCard
                key={`${provider.provider_id}-${provider.account_index ?? "default"}`}
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
          {t("widget.updated", {
            time: dashboard ? formatRefreshedAgo(dashboard.generated_at, now, t) : t("widget.time.never"),
          })}
        </span>
        <span className="w-footer-spacer" />
        {fetchedProviders.length > 0 && (
          <span className="w-footer-count">
            {t("widget.providerCount", {
              visible: String(visibleProviders.length),
              total: String(fetchedProviders.length),
            })}
          </span>
        )}
        <span className="w-footer-interval">
          <span className="w-dot" aria-hidden="true" />
          {t("widget.pollInterval", { seconds: String(POLL_INTERVAL_MS / 1000) })}
        </span>
      </footer>
    </div>
  );
}
