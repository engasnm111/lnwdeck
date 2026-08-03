import React from "react";
import ReactDOM from "react-dom/client";
import { FloatingWidget } from "./FloatingWidget";
import "./FloatingWidget.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <FloatingWidget />
  </React.StrictMode>,
);
