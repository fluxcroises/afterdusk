import { create } from "zustand";

/** Échec d'une tâche de fond, envoyé par le cœur (`background-warning`). */
export interface BackgroundWarning {
  kind: "xmp" | "xmpRecovered" | "preview";
  count: number;
  detail: string;
}

interface WarningsState {
  /** Photos dont les marques attendent leur XMP ; 0 quand tout est écrit. */
  xmpFailed: number;
  xmpDetail: string;
  /** Aperçus impossibles à créer depuis le dernier masquage. */
  previewFailed: number;
  previewDetail: string;
  onWarning: (w: BackgroundWarning) => void;
  dismissPreviews: () => void;
}

export const useWarnings = create<WarningsState>()((set) => ({
  xmpFailed: 0,
  xmpDetail: "",
  previewFailed: 0,
  previewDetail: "",
  onWarning: (w) => {
    switch (w.kind) {
      case "xmp":
        set({ xmpFailed: w.count, xmpDetail: w.detail });
        break;
      case "xmpRecovered":
        set({ xmpFailed: 0, xmpDetail: "" });
        break;
      case "preview":
        set((s) => ({ previewFailed: s.previewFailed + w.count, previewDetail: w.detail }));
        break;
    }
  },
  dismissPreviews: () => set({ previewFailed: 0, previewDetail: "" }),
}));
