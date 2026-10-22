import { invoke, isTauri } from "@tauri-apps/api/core";
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { installDesktopBehavior } from "./desktop";
import "./i18n";
import { installMenu } from "./menu";
import { applyPlatform } from "./platform";
import { hideSplash, removeSplash } from "./splash";
import { SecondaryApp } from "./secondary/SecondaryApp";
import { watchProfile } from "./store/profile";
import { WelcomeApp } from "./welcome/WelcomeApp";
import { currentWindow, showSelf, welcomeOnStartup } from "./windows";

const label = currentWindow();
// La fenêtre qui ouvre l'application s'affiche aussitôt, avec l'écran de démarrage ;
// l'autre reste masquée jusqu'à ce qu'on la demande.
const opensFirst = label !== "secondary" && (label === "welcome") === welcomeOnStartup();

applyPlatform();
installDesktopBehavior(label);
if (label === "main") installMenu();
if (isTauri()) void watchProfile();
if (opensFirst) void showSelf();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {label === "welcome" ? <WelcomeApp /> : label === "secondary" ? <SecondaryApp /> : <App />}
  </React.StrictMode>,
);

if (opensFirst || !isTauri()) hideSplash();
else removeSplash();

// L'interface est affichée (après la première image) : le cœur peut le noter,
// et l'essai de lancement de la CI s'arrête là.
if (opensFirst && isTauri()) {
  requestAnimationFrame(() =>
    requestAnimationFrame(() => void invoke("app_ready").catch(() => undefined)),
  );
}
