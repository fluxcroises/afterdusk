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
  toggleSection: (id: string) => void;
}

export const useLayout = create<LayoutState>((set) => ({
  visible: { left: true, right: true, filmstrip: true },
  collapsed: {},
  dropping: false,
  setDropping: (dropping) => set({ dropping }),
  togglePanel: (panel) => set((s) => ({ visible: { ...s.visible, [panel]: !s.visible[panel] } })),
  toggleSection: (id) => set((s) => ({ collapsed: { ...s.collapsed, [id]: !s.collapsed[id] } })),
}));
