import { create } from "zustand";

export type Panel = "left" | "right" | "filmstrip";

interface LayoutState {
  visible: Record<Panel, boolean>;
  /** Sections repliées, par identifiant (« cull.left.albums »). */
  collapsed: Record<string, boolean>;
  togglePanel: (panel: Panel) => void;
  toggleSection: (id: string) => void;
}

export const useLayout = create<LayoutState>((set) => ({
  visible: { left: true, right: true, filmstrip: true },
  collapsed: {},
  togglePanel: (panel) => set((s) => ({ visible: { ...s.visible, [panel]: !s.visible[panel] } })),
  toggleSection: (id) => set((s) => ({ collapsed: { ...s.collapsed, [id]: !s.collapsed[id] } })),
}));
