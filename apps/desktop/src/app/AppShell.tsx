import { useCallback, useEffect, useState } from "react";
import { Link, NavLink, Outlet, useLocation } from "react-router";
import { Badge, Button } from "@lnwdeck/ui";
import {
  AlertsIcon,
  AnalyticsIcon,
  BudgetsIcon,
  CostsIcon,
  ModelsIcon,
  OverviewIcon,
  ProvidersIcon,
  RefreshIcon,
  SettingsIcon,
  SystemIcon,
} from "../components/Icons";
import { UpdateNotification } from "../components/UpdateNotification";
import { fetchAlerts, fetchSettings, refreshAll } from "../lib/native";
import { formatRelativeTime, freshnessOf } from "../lib/freshness";

const navItems = [
  { to: "/", label: "Overview", icon: OverviewIcon },
  { to: "/providers", label: "Providers", icon: ProvidersIcon },
  { to: "/analytics", label: "Analytics", icon: AnalyticsIcon },
  { to: "/costs", label: "Costs", icon: CostsIcon },
  { to: "/budgets", label: "Budgets", icon: BudgetsIcon },
  { to: "/models", label: "Models", icon: ModelsIcon },
  { to: "/alerts", label: "Alerts", icon: AlertsIcon },
  { to: "/settings", label: "Settings", icon: SettingsIcon },
  { to: "/system", label: "System", icon: SystemIcon },
];

/**
 * Application shell.
 *
 * The freshness indicator reflects the last successful collection reported by
 * the backend. When no collection has succeeded yet it says so; it never shows
 * a "Fresh" badge based on the time the window happened to open.
 */
export function AppShell() {
  const [collapsed, setCollapsed] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [refreshError, setRefreshError] = useState<string | null>(null);
  const [lastSync, setLastSync] = useState<string | null>(null);
  const [appVersion, setAppVersion] = useState<string | null>(null);
  const [openAlerts, setOpenAlerts] = useState<number | null>(null);
  const [theme, setTheme] = useState<string>("system");
  const [now, setNow] = useState(() => Date.now());
  const location = useLocation();

  const currentNav =
    navItems.find((item) => item.to === location.pathname) ?? navItems[0];

  const loadStatus = useCallback(async () => {
    // Both calls are independent: a failure in one must not hide the other.
    try {
      const alerts = await fetchAlerts();
      setOpenAlerts(alerts.open_count);
    } catch {
      setOpenAlerts(null);
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
    const tick = setInterval(() => setNow(Date.now()), 30_000);
    return () => clearInterval(tick);
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
      setRefreshError(
        error instanceof Error ? error.message : "refresh failed",
      );
    } finally {
      setRefreshing(false);
    }
  }, [loadFreshness, loadStatus]);

  const freshness = freshnessOf(lastSync, now);

  return (
    <div className="app-layout">
      <nav
        aria-label="Main navigation"
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
              aria-label={collapsed ? "Expand navigation" : "Collapse navigation"}
              aria-expanded={!collapsed}
            >
              {collapsed ? "[+]" : "[-]"}
            </button>
          </div>
          <ul className="app-sidebar-nav">
            {navItems.map((item) => {
              const Icon = item.icon;
              const badgeCount =
                item.to === "/alerts" && openAlerts ? openAlerts : null;
              return (
                <li key={item.to}>
                  <NavLink
                    to={item.to}
                    end={item.to === "/"}
                    title={item.label}
                    className={({ isActive }) =>
                      `app-sidebar-link ${isActive ? "active" : ""}`.trim()
                    }
                  >
                    <Icon />
                    {!collapsed && <span>{item.label}</span>}
                    {badgeCount !== null && (
                      <span
                        className="app-sidebar-link-count"
                        aria-label={`${badgeCount} open alerts`}
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
          <Badge tone="neutral" title="All data stays on this machine">
            Local
          </Badge>
        </div>
      </nav>

      <div className="app-main-container">
        <UpdateNotification />
        {refreshError && (
          <div className="banner banner-error" role="alert">
            <span className="banner-body">Refresh failed: {refreshError}</span>
            <button
              type="button"
              className="banner-dismiss"
              onClick={() => setRefreshError(null)}
              aria-label="Dismiss refresh error"
            >
              x
            </button>
          </div>
        )}
        <header className="app-topbar">
          <h1 className="app-topbar-title">{currentNav.label}</h1>
          <div className="app-topbar-actions">
            <span className="app-freshness">
              <span>
                {lastSync
                  ? `Collected ${formatRelativeTime(lastSync, now)}`
                  : "No collection has succeeded yet"}
              </span>
              <Badge tone={freshness.tone}>{freshness.label}</Badge>
            </span>
            <Button
              variant="secondary"
              onClick={() => void handleGlobalRefresh()}
              disabled={refreshing}
              aria-label="Refresh all providers"
            >
              <RefreshIcon />
              {refreshing ? "Refreshing" : "Refresh all"}
            </Button>
          </div>
        </header>

        <main className="app-content">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
