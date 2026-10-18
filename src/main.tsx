import { isTauri } from "@tauri-apps/api/core";
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { installDesktopBehavior } from "./desktop";
import "./i18n";
import { installMenu } from "./menu";
import { hideSplash, removeSplash } from "./splash";
import { WelcomeApp } from "./welcome/WelcomeApp";
import { currentWindow, showSelf, welcomeOnStartup } from "./windows";

const label = currentWindow();
// La fenêtre qui ouvre l'application s'affiche aussitôt, avec l'écran de démarrage ;
// l'autre reste masquée jusqu'à ce qu'on la demande.
const opensFirst = (label === "welcome") === welcomeOnStartup();

installDesktopBehavior();
if (label === "main") installMenu();
if (opensFirst) void showSelf();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>{label === "welcome" ? <WelcomeApp /> : <App />}</React.StrictMode>,
);

if (opensFirst || !isTauri()) hideSplash();
else removeSplash();
