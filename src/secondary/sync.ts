import { emitTo, listen } from "@tauri-apps/api/event";
import { type MediaSummary, compareIds, useLibrary } from "../store/library";

/** Ce que la fenêtre principale envoie au second écran. */
export interface SecondaryState {
  /** Photo active de la fenêtre principale. */
  activeId: number | null;
  /** Photos de la comparaison (2 à 4), dans l'ordre de la grille. */
  compare: number[];
  /** Photos utiles : active, comparaison et voisines (préchargement). */
  photos: MediaSummary[];
  versions: Record<number, number>;
}

export const EVENT_STATE = "secondary-state";
export const EVENT_READY = "secondary-ready";

type LibrarySnapshot = Pick<
  ReturnType<typeof useLibrary.getState>,
  "media" | "selectedId" | "selectedIds" | "versions"
>;

export function secondaryState(s: LibrarySnapshot): SecondaryState {
  const index = s.media.findIndex((m) => m.id === s.selectedId);
  const compare = compareIds(s);
  const wanted = new Set(compare);
  for (const d of [0, 1, -1, 2, -2]) {
    const m = s.media[index + d];
    if (m) wanted.add(m.id);
  }
  const photos = s.media.filter((m) => wanted.has(m.id));
  const versions: Record<number, number> = {};
  for (const m of photos) versions[m.id] = s.versions[m.id] ?? 0;
  return { activeId: s.selectedId, compare, photos, versions };
}

/**
 * Fenêtre principale : envoie l'état au second écran à chaque changement
 * utile (une fois par image d'animation au plus), et sur demande à son ouverture.
 */
export function startSecondarySync(): () => void {
  let pending = false;
  const send = () => {
    pending = false;
    void emitTo("secondary", EVENT_STATE, secondaryState(useLibrary.getState()));
  };
  const schedule = () => {
    if (pending) return;
    pending = true;
    requestAnimationFrame(send);
  };
  const unsubscribe = useLibrary.subscribe((s, prev) => {
    if (
      s.selectedId !== prev.selectedId ||
      s.selectedIds !== prev.selectedIds ||
      s.media !== prev.media ||
      s.versions !== prev.versions
    ) {
      schedule();
    }
  });
  const ready = listen(EVENT_READY, send);
  return () => {
    unsubscribe();
    void ready.then((unlisten) => unlisten());
  };
}
