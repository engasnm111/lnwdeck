import React from "react";
import ReactDOM from "react-dom/client";
import { DesktopPet } from "./DesktopPet";
import { I18nProvider } from "../../app/I18nProvider";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <I18nProvider>
      <DesktopPet />
    </I18nProvider>
  </React.StrictMode>,
);
