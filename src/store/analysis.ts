import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";
import { toCommandError } from "../api/core";
import { useLibrary } from "./library";

const AUTO_KEY = "afterdusk.analysis.auto";

/** Préférence locale : analyser les photos après chaque import (oui par défaut). */
function readAuto(): boolean {
  try {
    return window.localStorage.getItem(AUTO_KEY) !== "false";
  } catch {
    return true;
  }
}

/** Raisons d'un raté probable, telles que le cœur les écrit. */
export type Miss = "blur" | "under" | "over" | "eyes";

export function missesOf(text: string | undefined): Miss[] {
  return (text ?? "").split(" ").filter(Boolean) as Miss[];
}

export interface RunReport {
  analyzed: number;
  misses: number;
  skipped: number;
}

interface AnalysisState {
  auto: boolean;
  running: boolean;
  progress: { done: number; total: number } | null;
  lastReport: RunReport | null;
  setAuto: (auto: boolean) => void;
  onProgress: (progress: { done: number; total: number }) => void;
  /** Analyse les photos pas encore vues : la bibliothèque, ou l'album affiché. */
  run: (albumId?: number | null) => Promise<void>;
  cancel: () => Promise<void>;
}

export const useAnalysis = create<AnalysisState>()((set, get) => ({
  auto: readAuto(),
  running: false,
  progress: null,
  lastReport: null,

  setAuto: (auto) => {
    set({ auto });
    try {
      window.localStorage.setItem(AUTO_KEY, String(auto));
    } catch {
      // Préférence de confort : sans stockage, elle vaut pour la session.
    }
  },

  onProgress: (progress) => set({ progress }),

  run: async (albumId = null) => {
    const libraryId = useLibrary.getState().libraryId;
    if (libraryId === null || get().running) return;
    set({ running: true, progress: null });
    try {
      const report = await invoke<RunReport>("analysis_run", { libraryId, albumId });
      set({ lastReport: report });
      // Les badges apparaissent ; le filtre « Ratés probables » s'en sert.
      await useLibrary.getState().reloadMedia();
    } catch (e) {
      useLibrary.setState({ error: toCommandError(e) });
    } finally {
      set({ running: false, progress: null });
    }
  },

  cancel: async () => {
    await invoke("analysis_cancel");
  },
}));
