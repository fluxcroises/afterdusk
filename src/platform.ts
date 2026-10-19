import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

/** Système d'exploitation, pour les quelques différences d'interface. */
export const isMac = typeof navigator !== "undefined" && navigator.userAgent.includes("Mac");

/**
 * Sous Windows, la fenêtre n'a pas de cadre natif : Afterdusk dessine sa barre
 * de titre, ses menus et ses boutons de fenêtre (comme Capture One).
 * Sous macOS, la barre de menus reste celle du système.
 */
export const drawsOwnMenus = !isMac;

/**
 * Attributs sur <html> pour les ajustements de mise en page propres au
 * système. Sous macOS, `data-fullscreen` suit le plein écran, où les trois
 * pastilles de la barre de titre disparaissent avec la place qui leur revient.
 */
export function applyPlatform(): void {
  const root = document.documentElement;
  root.dataset.platform = isMac ? "mac" : "windows";
  if (!isMac || !isTauri()) return;
  const win = getCurrentWindow();
  const update = async () => {
    root.dataset.fullscreen = String(await win.isFullscreen());
  };
  void update();
  void win.onResized(() => void update());
}
