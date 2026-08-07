import { useCallback, useEffect, useState } from "react";
import { Link, NavLink, Outlet, useLocation } from "react-router";
import { listen } from "@tauri-apps/api/event";
import { Badge, Button } from "@lnwdeck/ui";
import {
  AlertsIcon,
  AnalyticsIcon,
  BudgetsIcon,
  CostsIcon,
  ModelsIcon,
  OverviewIcon,
  PetIcon,
  ProvidersIcon,
  RefreshIcon,
  SettingsIcon,
  SystemIcon,
} from "../components/Icons";
import { UpdateNotification } from "../components/UpdateNotification";
import { fetchAlerts, fetchSettings, refreshAll } from "../lib/native";
import { formatRelativeTime, freshnessOf } from "../lib/freshness";
import { useI18n } from "../lib/i18n";
import { ALERTS_UPDATED_EVENT } from "../lib/ui-events";

const navItems = [
  { to: "/", key: "nav.overview", icon: OverviewIcon },
  { to: "/providers", key: "nav.providers", icon: ProvidersIcon },
  { to: "/analytics", key: "nav.analytics", icon: AnalyticsIcon },
  { to: "/costs", key: "nav.costs", icon: CostsIcon },
  { to: "/budgets", key: "nav.budgets", icon: BudgetsIcon },
  { to: "/models", key: "nav.models", icon: ModelsIcon },
  { to: "/alerts", key: "nav.alerts", icon: AlertsIcon },
  { to: "/pet", key: "nav.pet", icon: PetIcon },
  { to: "/settings", key: "nav.settings", icon: SettingsIcon },
  { to: "/system", key: "nav.system", icon: SystemIcon },
];

/**
 * Application shell.
 *
 * The freshness indicator reflects the last successful collection reported by
 * the backend. When no collection has succeeded yet it says so; it never shows
 * a "Fresh" badge based on the time the window happened to open.
 */
export function AppShell() {
  const { t, language } = useI18n();
  const [collapsed, setCollapsed] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [refreshError, setRefreshError] = useState<string | null>(null);
  const [lastSync, setLastSync] = useState<string | null>(null);
  const [appVersion, setAppVersion] = useState<string | null>(null);
  const [unacknowledgedAlerts, setUnacknowledgedAlerts] = useState<number | null>(null);
  const [theme, setTheme] = useState<string>("system");
  const [now, setNow] = useState(() => Date.now());
  const location = useLocation();

  const currentNav =
    navItems.find((item) => item.to === location.pathname) ?? navItems[0];

  const loadStatus = useCallback(async () => {
    // Both calls are independent: a failure in one must not hide the other.
    try {
      const alerts = await fetchAlerts();
      setUnacknowledgedAlerts(alerts.unacknowledged_count);
    } catch {
      setUnacknowledgedAlerts(null);
    }
    try {
      const view = await fetchSettings();
      setTheme(view.settings.theme);
    } catch {
      setTheme("system");
    }
  }, []);

  const loadFreshness = useCallback(async () => {
    try {
      const { fetchPipelineDiagnostics } = await import("../lib/native");
      const diagnostics = await fetchPipelineDiagnostics();
      setLastSync(diagnostics.totals.last_successful_sync);
      setAppVersion(diagnostics.app_version);
    } catch {
      setLastSync(null);
    }
  }, []);

  useEffect(() => {
    void loadStatus();
    void loadFreshness();
    const unlisten = listen<string>("settings-changed", (event) => {
      setTheme(event.payload);
    });
    const onAlertsUpdated = () => {
      void loadStatus();
    };
    window.addEventListener(ALERTS_UPDATED_EVENT, onAlertsUpdated);
    const tick = setInterval(() => setNow(Date.now()), 30_000);
    return () => {
      clearInterval(tick);
      void unlisten.then((fn) => fn());
      window.removeEventListener(ALERTS_UPDATED_EVENT, onAlertsUpdated);
    };
  }, [loadStatus, loadFreshness]);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
  }, [theme]);

  const handleGlobalRefresh = useCallback(async () => {
    setRefreshing(true);
    setRefreshError(null);
    try {
      await refreshAll();
      await loadFreshness();
      await loadStatus();
      setNow(Date.now());
    } catch (error) {
      const message = error instanceof Error ? error.message : "refresh failed";
      // A cycle is already running (background or a previous click): the UI
      // stays responsive and keeps showing the refresh state instead of an
      // error that would invite more clicking.
      if (!/already in progress/i.test(message)) {
        setRefreshError(message);
      }
    } finally {
      setRefreshing(false);
    }
  }, [loadFreshness, loadStatus]);

  const freshness = freshnessOf(lastSync, now);

  return (
    <>
      <div className="app-backdrop" aria-hidden="true">
        <span className="app-backdrop-orb app-backdrop-orb-cyan" />
        <span className="app-backdrop-orb app-backdrop-orb-blue" />
        <span className="app-backdrop-orb app-backdrop-orb-violet" />
      </div>
      <div className="app-layout">
      <nav
        aria-label={t("app.navAria")}
        className={`app-sidebar ${collapsed ? "app-sidebar-collapsed" : ""}`.trim()}
      >
        <div>
          <div className="app-sidebar-brand">
            {!collapsed && (
              <Link to="/" className="app-sidebar-brand-name">
                <span className="app-sidebar-brand-mark" aria-hidden="true" />
                lnwdeck
              </Link>
            )}
            <button
              type="button"
              className="app-sidebar-collapse"
              onClick={() => setCollapsed((value) => !value)}
              aria-label={
                collapsed ? t("app.expandNav") : t("app.collapseNav")
              }
              aria-expanded={!collapsed}
            >
              {collapsed ? "[+]" : "[-]"}
            </button>
          </div>
          <ul className="app-sidebar-nav">
            {navItems.map((item) => {
              const Icon = item.icon;
              const badgeCount =
                item.to === "/alerts" && unacknowledgedAlerts
                  ? unacknowledgedAlerts
                  : null;
              return (
                <li key={item.to}>
                  <NavLink
                    to={item.to}
                    end={item.to === "/"}
                    title={t(item.key)}
                    className={({ isActive }) =>
                      `app-sidebar-link ${isActive ? "active" : ""}`.trim()
                    }
                  >
                    <Icon />
                    {!collapsed && <span>{t(item.key)}</span>}
                    {badgeCount !== null && (
                      <span
                        className="app-sidebar-link-count"
                        aria-label={t("app.openAlerts", {
                          count: String(badgeCount),
                        })}
                      >
                        {badgeCount}
                      </span>
                    )}
                  </NavLink>
                </li>
              );
            })}
          </ul>
        </div>
        <div className="app-sidebar-footer">
          {!collapsed && appVersion && <span>v{appVersion}</span>}
          <Badge tone="neutral" title={t("app.localHint")}>
            {t("app.local")}
          </Badge>
        </div>
      </nav>

      <div className="app-main-container">
        <UpdateNotification />
        {refreshError && (
          <div className="banner banner-error" role="alert">
            <span className="banner-body">
              {t("app.refreshFailed", { error: refreshError })}
            </span>
            <button
              type="button"
              className="banner-dismiss"
              onClick={() => setRefreshError(null)}
              aria-label={t("app.dismiss")}
            >
              x
            </button>
          </div>
        )}
        <header className="app-topbar">
          <h1 className="app-topbar-title">{t(currentNav.key)}</h1>
          <div className="app-topbar-actions">
            <span className="app-freshness">
              <span>
                {lastSync
                  ? t("topbar.collected", { time: formatRelativeTime(lastSync, now, language) })
                  : t("topbar.noCollection")}
              </span>
              <Badge tone={freshness.tone}>{freshness.label}</Badge>
            </span>
            <Button
              variant="secondary"
              onClick={() => void handleGlobalRefresh()}
              disabled={refreshing}
              aria-label={
                refreshing ? t("topbar.refreshing") : t("topbar.refresh")
              }
              data-refreshing={refreshing || undefined}
            >
              <RefreshIcon />
              {refreshing ? t("topbar.refreshing") : t("topbar.refresh")}
            </Button>
          </div>
        </header>

        <main className="app-content">
          <Outlet />
        </main>
      </div>
      </div>
    </>
  );
}
