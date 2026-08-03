import { useState, useCallback } from "react";
import { Link, NavLink, Outlet, useLocation } from "react-router-dom";
import { Badge, Button } from "@lnwdeck/ui";
import {
  OverviewIcon,
  ProvidersIcon,
  AnalyticsIcon,
  CostsIcon,
  BudgetsIcon,
  ModelsIcon,
  AlertsIcon,
  SettingsIcon,
  SystemIcon,
  RefreshIcon,
} from "../components/Icons";
import { refreshAll } from "../lib/native";

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

export function AppShell() {
  const [collapsed, setCollapsed] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [lastUpdated, setLastUpdated] = useState<string>(new Date().toLocaleTimeString());
  const location = useLocation();

  const currentNav = navItems.find((item) => item.to === location.pathname) || navItems[0];

  const handleGlobalRefresh = useCallback(async () => {
    setRefreshing(true);
    try {
      await refreshAll();
      setLastUpdated(new Date().toLocaleTimeString());
    } catch {
      // ignore
    } finally {
      setRefreshing(false);
    }
  }, []);

  return (
    <div className="app-layout">
      <nav
        role="navigation"
        aria-label="Main navigation"
        className={`app-sidebar ${collapsed ? "app-sidebar-collapsed" : ""}`}
      >
        <div>
          <div className="app-sidebar-brand">
            <h1 style={{ margin: 0, fontSize: "1.25rem", fontWeight: 700 }}>
              <Link to="/" style={{ color: "inherit", textDecoration: "none" }}>
                {collapsed ? "lnw" : "lnwdeck"}
              </Link>
            </h1>
            <button
              type="button"
              onClick={() => setCollapsed((c) => !c)}
              aria-label="Toggle sidebar"
              style={{
                marginLeft: "auto",
                background: "transparent",
                border: "none",
                color: "var(--text-muted)",
              }}
            >
              {collapsed ? "→" : "←"}
            </button>
          </div>
          <ul className="app-sidebar-nav">
            {navItems.map((item) => {
              const Icon = item.icon;
              return (
                <li key={item.to}>
                  <NavLink
                    to={item.to}
                    end={item.to === "/"}
                    className={({ isActive }) =>
                      `app-sidebar-link ${isActive ? "active" : ""}`
                    }
                  >
                    <Icon />
                    {!collapsed && <span>{item.label}</span>}
                  </NavLink>
                </li>
              );
            })}
          </ul>
        </div>
        <div className="app-sidebar-footer">
          {!collapsed && <span>v0.1.0</span>}
          <Badge tone="success">Local</Badge>
        </div>
      </nav>

      <div className="app-main-container">
        <header className="app-topbar">
          <h1 className="app-topbar-title">{currentNav.label}</h1>
          <div className="app-topbar-actions">
            <span style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>
              Updated {lastUpdated}
            </span>
            <Badge tone="success">Fresh</Badge>
            <Button
              variant="secondary"
              onClick={handleGlobalRefresh}
              disabled={refreshing}
              aria-label="Refresh All"
            >
              <RefreshIcon />
              {refreshing ? "Refreshing…" : "Refresh All"}
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
