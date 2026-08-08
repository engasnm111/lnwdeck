import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { fetchOverview, OverviewData } from "../../lib/native";
import { TokenValue } from "../../components/TokenValue";
import { formatFullTokenCount } from "../../lib/token-format";
import { useI18n } from "../../lib/i18n";
import "./TrayPopup.css";

export function TrayPopup() {
  const { t } = useI18n();
  const [data, setData] = useState<OverviewData | null>(null);

  const load = useCallback(async () => {
    try {
      const result = await fetchOverview();
      setData(result);
    } catch {
      // Silently handle fallback in tray popup
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const handleOpenDashboard = async () => {
    try {
      await invoke("show_main_window");
      await invoke("hide_tray_popup");
    } catch {
      // Fallback
    }
  };

  const totalTokens = data
    ? data.total_tokens_input + data.total_tokens_output
    : 0;

  return (
    <div className="tray-window">
      <div className="tray-card">
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
          <span className="tray-badge-ok">{t("tray.ok")}</span>
        </div>

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
              <span className="tray-metric-value">$0.00</span>
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
          className="tray-action-btn"
          onClick={handleOpenDashboard}
        >
          {t("tray.openDashboard")}
        </button>
      </div>

      {/* Footer Status Bar */}
      <div className="tray-footer-bar">
        <span className="tray-footer-status">{t("tray.running")}</span>
        <span className="tray-badge-lnwdev">LNWDEV</span>
      </div>
    </div>
  );
}
