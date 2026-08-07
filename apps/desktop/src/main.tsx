import React from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter } from "react-router";
import App from "./App";
import { I18nProvider } from "./app/I18nProvider";
import "./styles/global.css";
import "./styles/layout.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <I18nProvider>
      <BrowserRouter>
        <App />
      </BrowserRouter>
    </I18nProvider>
  </React.StrictMode>,
);
