import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { create } from "zustand";

export type Panel = "left" | "right" | "filmstrip";

interface LayoutState {
  visible: Record<Panel, boolean>;
  /** Sections repliées, par identifiant (« cull.left.albums »). */
  collapsed: Record<string, boolean>;
  /** Un fichier ou dossier est glissé au-dessus de la fenêtre. */
  dropping: boolean;
  setDropping: (dropping: boolean) => void;
  togglePanel: (panel: Panel) => void;
  /** Ouvre ou replie une section ; `collapsed` : son état affiché. */
  toggleSection: (id: string, collapsed: boolean) => void;
  /**
   * Plein écran de la visionneuse (F) : panneaux, barres et pellicule masqués,
   * fenêtre en plein écran. Échap en sort.
   */
  immersive: boolean;
  setImmersive: (value: boolean) => void;
  /** Barre de filtres au-dessus de la grille (\). */
  filterBar: boolean;
  setFilterBar: (value: boolean) => void;
}

const FILTER_BAR_KEY = "afterdusk.filterBar";

function readFilterBar(): boolean {
  try {
    return window.localStorage.getItem(FILTER_BAR_KEY) !== "false";
  } catch {
    return true;
  }
}

export const useLayout = create<LayoutState>((set) => ({
  visible: { left: true, right: true, filmstrip: true },
  collapsed: {},
  dropping: false,
  setDropping: (dropping) => set({ dropping }),
  immersive: false,
  setImmersive: (value) => {
    set({ immersive: value });
    if (isTauri()) void getCurrentWindow().setFullscreen(value);
  },
  filterBar: readFilterBar(),
  setFilterBar: (value) => {
    set({ filterBar: value });
    try {
      window.localStorage.setItem(FILTER_BAR_KEY, String(value));
    } catch {
      // Préférence de confort : sans stockage, elle vaut pour la session.
    }
  },
  togglePanel: (panel) => set((s) => ({ visible: { ...s.visible, [panel]: !s.visible[panel] } })),
  toggleSection: (id, collapsed) =>
    set((s) => ({ collapsed: { ...s.collapsed, [id]: !collapsed } })),
}));
