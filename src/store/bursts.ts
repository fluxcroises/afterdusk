import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";
import { toCommandError } from "../api/core";
import { useLibrary } from "./library";

const AUTO_KEY = "afterdusk.bursts.auto";

/** Préférence locale : regrouper les rafales après chaque import (oui par défaut). */
function readAuto(): boolean {
  try {
    return window.localStorage.getItem(AUTO_KEY) !== "false";
  } catch {
    return true;
  }
}

export interface DetectReport {
  bursts: number;
  photos: number;
}

interface BurstsState {
  auto: boolean;
  running: boolean;
  /** Dernière détection : rafales trouvées et photos rangées. */
  lastReport: DetectReport | null;
  setAuto: (auto: boolean) => void;
  /** Recalcule les rafales de la bibliothèque, ou de l'album affiché. */
  detect: (albumId?: number | null) => Promise<void>;
  /** Empile la sélection (deux photos au moins) ; la photo active passe au-dessus. */
  stackSelection: () => Promise<void>;
  /** Retire la sélection de sa rafale. */
  unstackSelection: () => Promise<void>;
}

export const useBursts = create<BurstsState>()((set, get) => {
  async function guard(run: () => Promise<void>) {
    try {
      await run();
    } catch (e) {
      useLibrary.setState({ error: toCommandError(e) });
    }
  }

  return {
    auto: readAuto(),
    running: false,
    lastReport: null,

    setAuto: (auto) => {
      set({ auto });
      try {
        window.localStorage.setItem(AUTO_KEY, String(auto));
      } catch {
        // Préférence de confort : sans stockage, elle vaut pour la session.
      }
    },

    detect: (albumId = null) =>
      guard(async () => {
        const libraryId = useLibrary.getState().libraryId;
        if (libraryId === null || get().running) return;
        set({ running: true });
        try {
          const report = await invoke<DetectReport>("bursts_detect", { libraryId, albumId });
          set({ lastReport: report });
          await useLibrary.getState().reloadMedia();
        } finally {
          set({ running: false });
        }
      }),

    stackSelection: () =>
      guard(async () => {
        const { libraryId, selectedId, targetIds } = useLibrary.getState();
        const ids = targetIds().filter((id) => id > 0);
        if (libraryId === null || ids.length < 2) return;
        // La photo active d'abord : elle passe au-dessus.
        const ordered =
          selectedId !== null && ids.includes(selectedId)
            ? [selectedId, ...ids.filter((id) => id !== selectedId)]
            : ids;
        await invoke("bursts_stack", { libraryId, ids: ordered });
        await useLibrary.getState().reloadMedia();
      }),

    unstackSelection: () =>
      guard(async () => {
        const ids = useLibrary
          .getState()
          .targetIds()
          .filter((id) => id > 0);
        if (ids.length === 0) return;
        await invoke("bursts_unstack", { ids });
        await useLibrary.getState().reloadMedia();
      }),
  };
});
