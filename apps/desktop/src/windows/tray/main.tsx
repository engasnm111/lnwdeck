import React from "react";
import ReactDOM from "react-dom/client";
import { I18nProvider } from "../../app/I18nProvider";
import { TrayPopup } from "./TrayPopup";
import "../../styles/global.css";
import "./TrayPopup.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <I18nProvider>
      <TrayPopup />
    </I18nProvider>
  </React.StrictMode>,
);
