import { Route, Routes } from "react-router";
import { AppShell } from "./app/AppShell";
import { OverviewPage } from "./pages/OverviewPage";
import { ProvidersPage } from "./pages/ProvidersPage";
import { AnalyticsPage } from "./pages/analytics/AnalyticsPage";
import { CostsPage } from "./pages/costs/CostsPage";
import { BudgetsPage } from "./pages/budgets/BudgetsPage";
import { ModelsPage } from "./pages/models/ModelsPage";
import { SessionsPage } from "./pages/sessions/SessionsPage";
import { AlertsPage } from "./pages/alerts/AlertsPage";
import { PetPage } from "./pages/pet/PetPage";
import { SettingsPage } from "./pages/settings/SettingsPage";
import { SystemPage } from "./pages/system/SystemPage";

/**
 * Dashboard routes.
 *
 * The widget and the tray popup are separate windows with their own HTML
 * entries, so they are deliberately not routed here.
 */
function App() {
  return (
    <Routes>
      <Route element={<AppShell />}>
        <Route index element={<OverviewPage />} />
        <Route path="providers" element={<ProvidersPage />} />
        <Route path="analytics" element={<AnalyticsPage />} />
        <Route path="costs" element={<CostsPage />} />
        <Route path="budgets" element={<BudgetsPage />} />
        <Route path="models" element={<ModelsPage />} />
        <Route path="sessions" element={<SessionsPage />} />
        <Route path="alerts" element={<AlertsPage />} />
        <Route path="pet" element={<PetPage />} />
        <Route path="settings" element={<SettingsPage />} />
        <Route path="system" element={<SystemPage />} />
      </Route>
    </Routes>
  );
}

export default App;
