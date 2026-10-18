import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { installDesktopBehavior } from "./desktop";
import "./i18n";
import { installMenu } from "./menu";

installDesktopBehavior();
installMenu();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
