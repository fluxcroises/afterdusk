/** Système d'exploitation, pour les quelques différences d'interface. */
export const isMac = typeof navigator !== "undefined" && navigator.userAgent.includes("Mac");

/**
 * Sous Windows, la fenêtre n'a pas de cadre natif : Afterdusk dessine sa barre
 * de titre, ses menus et ses boutons de fenêtre (comme Capture One).
 * Sous macOS, la barre de menus reste celle du système.
 */
export const drawsOwnMenus = !isMac;

/** Classe sur <html> pour les ajustements de mise en page propres au système. */
export function applyPlatform(): void {
  document.documentElement.dataset.platform = isMac ? "mac" : "windows";
}
