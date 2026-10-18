import { create } from "zustand";

export const STEPS = ["import", "cull", "develop", "deliver"] as const;
export const VIEWS = ["library", "map", "gear", "settings"] as const;

export type Step = (typeof STEPS)[number];
export type View = (typeof VIEWS)[number];
export type Screen = Step | View;

interface NavigationState {
  screen: Screen;
  goTo: (screen: Screen) => void;
}

export const useNavigation = create<NavigationState>((set) => ({
  screen: "import",
  goTo: (screen) => set({ screen }),
}));
