import { Routes, Route } from "react-router-dom";
import { AppShell } from "./app/AppShell";
import { OverviewPage } from "./pages/OverviewPage";
import { ProvidersPage } from "./pages/ProvidersPage";
import { AnalyticsPage } from "./pages/analytics/AnalyticsPage";
import { CostsPage } from "./pages/costs/CostsPage";
import { BudgetsPage } from "./pages/budgets/BudgetsPage";
import { ModelsPage } from "./pages/models/ModelsPage";
import { AlertsPage } from "./pages/alerts/AlertsPage";
import { SettingsPage } from "./pages/settings/SettingsPage";
import { SystemPage } from "./pages/system/SystemPage";
import { FloatingWidget } from "./windows/widget/FloatingWidget";

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
        <Route path="alerts" element={<AlertsPage />} />
        <Route path="settings" element={<SettingsPage />} />
        <Route path="system" element={<SystemPage />} />
      </Route>
      <Route path="widget" element={<FloatingWidget />} />
    </Routes>
  );
}

export default App;
