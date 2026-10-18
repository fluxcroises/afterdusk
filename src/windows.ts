import { isTauri } from "@tauri-apps/api/core";
import { emitTo } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import type { Screen } from "./store/navigation";

/** Fenêtres déclarées dans `tauri.conf.json`. */
export type WindowLabel = "main" | "welcome";

export const EVENT_OPEN_MAIN = "open-main";
export const EVENT_WELCOME_REFRESH = "welcome-refresh";

/** Ce que l'accueil demande à la fenêtre principale d'afficher. */
export interface MainAction {
  screen: Screen;
  albumId?: number | null;
  mediaId?: number;
  scanPath?: string;
  resumeImportId?: number;
}

/** Fenêtre courante ; dans un navigateur (développement), `?window=welcome`. */
export function currentWindow(): WindowLabel {
  if (isTauri()) return getCurrentWindow().label === "welcome" ? "welcome" : "main";
  return new URLSearchParams(window.location.search).get("window") === "welcome"
    ? "welcome"
    : "main";
}

const STARTUP_KEY = "afterdusk.welcomeOnStartup";

/** Préférence locale : ouvrir l'accueil au démarrage (oui par défaut). */
export function welcomeOnStartup(): boolean {
  try {
    return window.localStorage.getItem(STARTUP_KEY) !== "false";
  } catch {
    return true;
  }
}

export function setWelcomeOnStartup(value: boolean): void {
  try {
    window.localStorage.setItem(STARTUP_KEY, String(value));
  } catch {
    // Préférence de confort : sans stockage, l'accueil reste affiché.
  }
}

export async function showSelf(): Promise<void> {
  if (!isTauri()) return;
  const win = getCurrentWindow();
  await win.show();
  await win.unminimize();
  await win.setFocus();
}

/** Depuis l'accueil : ouvre la fenêtre principale sur l'action demandée. */
export async function openMain(action: MainAction): Promise<void> {
  if (!isTauri()) return;
  await emitTo("main", EVENT_OPEN_MAIN, action);
  await getCurrentWindow().hide();
}

/** Depuis la fenêtre principale : rouvre l'accueil, à jour. */
export async function showWelcome(): Promise<void> {
  if (!isTauri()) return;
  const welcome = await WebviewWindow.getByLabel("welcome");
  if (!welcome) return;
  await emitTo("welcome", EVENT_WELCOME_REFRESH);
  await welcome.show();
  await welcome.unminimize();
  await welcome.setFocus();
}
