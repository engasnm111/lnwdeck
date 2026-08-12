import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { fetchOverview, OverviewData } from "../../lib/native";
import { useDebouncedCallback } from "../../lib/use-debounced-reload";
import { useLatestRequestGuard } from "../../lib/use-latest-request-guard";
import { TokenValue } from "../../components/TokenValue";
import { formatFullTokenCount } from "../../lib/token-format";
import { useI18n } from "../../lib/i18n";
import "./TrayPopup.css";

type UpdateNotice =
  | { kind: "up-to-date"; version: string }
  | { kind: "failed"; error: string };

export function TrayPopup() {
  const { t } = useI18n();
  const [data, setData] = useState<OverviewData | null>(null);
  const [notice, setNotice] = useState<UpdateNotice | null>(null);
  const loadInFlight = useRef<Promise<void> | null>(null);
  const beginRequest = useLatestRequestGuard([]);

  const load = useCallback(async () => {
    if (loadInFlight.current) return loadInFlight.current;
    const isCurrent = beginRequest();
    const pending = (async () => {
      try {
        const result = await fetchOverview();
        if (isCurrent()) {
          setData(result);
        }
      } catch {
        // Silently handle fallback in tray popup
      }
    })();
    const tracked = pending.finally(() => {
      if (loadInFlight.current === tracked) {
        loadInFlight.current = null;
      }
    });
    loadInFlight.current = tracked;
    return tracked;
  }, [beginRequest]);

  const scheduleReload = useDebouncedCallback(() => {
    void load();
  }, 1200);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | null = null;
    void listen("usage-updated", () => {
      scheduleReload();
    })
      .then((cleanup) => {
        if (cancelled) {
          cleanup();
        } else {
          unlisten = cleanup;
        }
      })
      .catch(() => {
        // Outside a Tauri runtime there is no event bus.
      });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [scheduleReload]);

  // The tray's "Check for updates" menu item reports through these events.
  // An up-to-date check is a normal result, not a failure: the banner
  // explains it in the user's language and matches the popup theme.
  useEffect(() => {
    const unlisteners: Array<() => void> = [];
    const setup = async () => {
      try {
        unlisteners.push(
          await listen<{ version: string }>("update-up-to-date", (event) => {
            setNotice({ kind: "up-to-date", version: event.payload.version });
          }),
        );
        unlisteners.push(
          await listen<{ code: string }>("update-check-failed", (event) => {
            if (event.payload.code !== "UP_TO_DATE") {
              setNotice({ kind: "failed", error: event.payload.code });
            }
          }),
        );
      } catch {
        // Outside a Tauri runtime there is no event bus; the popup then only
        // shows its metrics.
      }
    };
    void setup();
    return () => {
      for (const unlisten of unlisteners) {
        unlisten();
      }
    };
  }, []);

  const handleOpenDashboard = async () => {
    try {
      await invoke("open_dashboard_from_tray");
    } catch {
      // Fallback
    }
  };

  const totalTokens = data
    ? data.total_tokens_input + data.total_tokens_output
    : 0;
  const formattedCost = data?.cost_formatted?.trim() ?? "";
  const costDisplay =
    data &&
    data.total_events > 0 &&
    !["no_data", "missing_pricing"].includes(data.cost_status) &&
    formattedCost &&
    formattedCost !== "$0.00"
      ? formattedCost
      : t("tray.costUnavailable");

  return (
    <div
      className="tray-window tray-window-flat tray-window-gradient"
      data-surface="opaque"
    >
      <div className="tray-card tray-card-flat tray-card-modern">
        {/* Header */}
        <div className="tray-header">
          <div className="tray-brand">
            <svg
              className="tray-brand-icon"
              viewBox="0 0 32 32"
              fill="none"
              xmlns="http://www.w3.org/2000/svg"
            >
              <defs>
                <linearGradient
                  id="featherGrad"
                  x1="0%"
                  y1="0%"
                  x2="100%"
                  y2="100%"
                >
                  <stop offset="0%" stopColor="#22d3ee" />
                  <stop offset="55%" stopColor="#2563eb" />
                  <stop offset="100%" stopColor="#8b5cf6" />
                </linearGradient>
              </defs>
              <path
                d="M26 6C18 6 10 12 8 26C14 24 22 20 26 6Z"
                fill="url(#featherGrad)"
              />
              <path
                d="M8 26C12 18 20 12 26 6"
                stroke="#ffffff"
                strokeWidth="1.5"
                strokeLinecap="round"
              />
            </svg>
            <span className="tray-brand-title">lnwdeck</span>
          </div>
          <span className="tray-badge-ok tray-badge-flat">{t("tray.ok")}</span>
        </div>

        {notice && (
          <div
            className={`tray-update-notice ${
              notice.kind === "failed"
                ? "tray-update-notice-error"
                : "tray-update-notice-ok"
            }`.trim()}
            role="alert"
            aria-live="polite"
          >
            <span className="tray-update-notice-icon" aria-hidden="true">
              {notice.kind === "failed" ? (
                <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
                  <path d="M2 2l6 6M8 2l-6 6" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
                </svg>
              ) : (
                <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
                  <path d="M1.5 5.2l2.3 2.3L8.5 2.5" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
                </svg>
              )}
            </span>
            <span className="tray-update-notice-text">
              {notice.kind === "failed"
                ? t("tray.updateCheckFailed", { error: notice.error })
                : t("tray.upToDateDetail", { version: notice.version })}
            </span>
            <button
              type="button"
              className="tray-update-notice-dismiss"
              onClick={() => setNotice(null)}
              aria-label={t("tray.dismiss")}
            >
              x
            </button>
          </div>
        )}

        {/* Metrics List */}
        {data ? (
          <div className="tray-metrics">
            {/* Total token metric */}
            <div className="tray-metric-row">
              <div className="tray-metric-left">
                <div className="tray-icon-container">
                  <span className="tray-icon-t">T</span>
                </div>
                <span className="tray-metric-label">{t("tray.totalTokens")}</span>
              </div>
              <TokenValue
                value={totalTokens}
                label={t("tray.totalTokens")}
                exactLabel={t("tray.totalTokensExact")}
                className="tray-metric-value"
              />
            </div>

            {/* Total Cost (Estimated) */}
            <div className="tray-metric-row">
              <div className="tray-metric-left">
                <div className="tray-icon-container">
                  <span className="tray-icon-dollar">$</span>
                </div>
                <span className="tray-metric-label">
                  {t("tray.costEstimated")}
                </span>
              </div>
              <span className="tray-metric-value">{costDisplay}</span>
            </div>

            {/* Requests */}
            <div className="tray-metric-row">
              <div className="tray-metric-left">
                <div className="tray-icon-container">
                  <svg
                    className="tray-icon-cube"
                    width="18"
                    height="18"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="2"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  >
                    <path d="m21 16-9 5-9-5V8l9-5 9 5v8z" />
                    <path d="m3.3 7 8.7 5 8.7-5" />
                    <path d="M12 12v9" />
                  </svg>
                </div>
                <span className="tray-metric-label">{t("tray.requests")}</span>
              </div>
              <span className="tray-metric-value">
                {formatFullTokenCount(data.total_events)}
              </span>
            </div>

            {/* Providers */}
            <div className="tray-metric-row">
              <div className="tray-metric-left">
                <div className="tray-icon-container">
                  <svg
                    className="tray-icon-chart"
                    width="18"
                    height="18"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="2"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  >
                    <line x1="18" y1="20" x2="18" y2="10" />
                    <line x1="12" y1="20" x2="12" y2="4" />
                    <line x1="6" y1="20" x2="6" y2="14" />
                  </svg>
                </div>
                <span className="tray-metric-label">{t("tray.providers")}</span>
              </div>
              <span className="tray-metric-value">
                {formatFullTokenCount(data.provider_count)}
              </span>
            </div>
          </div>
        ) : (
          <div className="tray-loading">{t("tray.loading")}</div>
        )}

        {/* Action Button */}
        <button
          type="button"
          className="tray-action-btn tray-action-btn-filled"
          onClick={handleOpenDashboard}
        >
          {t("tray.openDashboard")}
        </button>
      </div>

      {/* Footer Status Bar */}
      <div className="tray-footer-bar">
        <span className="tray-footer-status">{t("tray.running")}</span>
        <span className="tray-badge-lnwdev tray-badge-flat">LNWDEV</span>
      </div>
    </div>
  );
}
