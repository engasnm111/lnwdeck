import { Routes, Route } from "react-router-dom";
import { AppShell } from "./app/AppShell";
import { OverviewPage } from "./pages/OverviewPage";
import { ProvidersPage } from "./pages/ProvidersPage";

function App() {
  return (
    <Routes>
      <Route element={<AppShell />}>
        <Route index element={<OverviewPage />} />
        <Route path="providers" element={<ProvidersPage />} />
      </Route>
    </Routes>
  );
}

export default App;
