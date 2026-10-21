import { create } from "zustand";

/** Étapes du flux, dans l'ordre. */
export const STEPS = ["import", "cull", "develop", "deliver"] as const;
/** Vues transverses, à droite des étapes dans le sélecteur de modules. */
export const VIEWS = ["library", "map", "gear"] as const;

export type Step = (typeof STEPS)[number];
export type View = (typeof VIEWS)[number];
export type Screen = Step | View | "settings";

/** Version qui livre chaque écran ; les autres restent visibles mais signalés. */
export const AVAILABLE_IN: Record<Screen, string> = {
  import: "0.1",
  cull: "0.1",
  library: "0.1",
  settings: "0.1",
  develop: "0.2",
  deliver: "0.2",
  map: "0.3",
  gear: "0.3",
};

export const CURRENT_VERSION = "0.3";

/** Écran prévu pour une version postérieure à celle en cours. */
export function isLater(screen: Screen): boolean {
  return Number(AVAILABLE_IN[screen]) > Number(CURRENT_VERSION);
}

interface NavigationState {
  screen: Screen;
  goTo: (screen: Screen) => void;
}

export const useNavigation = create<NavigationState>((set) => ({
  screen: "import",
  goTo: (screen) => set({ screen }),
}));
