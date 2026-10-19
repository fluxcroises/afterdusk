import { useLayout } from "./store/layout";

/** Délai avant de lancer la recherche, le temps de finir le mot. */
export const SEARCH_DELAY_MS = 250;

/** Demande le champ de recherche (Ctrl+F) : la barre s'affiche s'il le faut. */
export const FOCUS_SEARCH_EVENT = "afterdusk:focus-search";

export function focusSearch(): void {
  useLayout.getState().setFilterBar(true);
  // Après l'affichage de la barre.
  window.setTimeout(() => window.dispatchEvent(new Event(FOCUS_SEARCH_EVENT)), 0);
}
