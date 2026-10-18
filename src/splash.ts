/** Durée minimale d'affichage, pour que l'animation aille à son terme. */
const MIN_DURATION_MS = 1700;
const FADE_MS = 320;

/** Fait disparaître l'écran de démarrage une fois l'interface prête. */
export function hideSplash(): void {
  const splash = document.getElementById("splash");
  if (!splash) return;
  const reduced = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
  const wait = Math.max(0, (reduced ? 400 : MIN_DURATION_MS) - performance.now());
  window.setTimeout(() => {
    splash.classList.add("splash-done");
    window.setTimeout(() => splash.remove(), FADE_MS);
  }, wait);
}

/** Retire l'écran de démarrage sans animation (fenêtre ouverte plus tard). */
export function removeSplash(): void {
  document.getElementById("splash")?.remove();
}
