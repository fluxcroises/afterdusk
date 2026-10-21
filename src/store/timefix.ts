import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";
import { type CommandError, toCommandError } from "../api/core";
import { useLibrary } from "./library";
import type { ClockInfo } from "./locate";

/** Photos d'un même lieu avec la même erreur d'horloge, miroir de `geo::time::TimeGroup`. */
export interface TimeGroup {
  city: string | null;
  cameraMinutes: number;
  localMinutes: number;
  shiftMinutes: number;
  count: number;
}

export interface TimeReport {
  groups: TimeGroup[];
  correct: number;
  noPlace: number;
  noZone: number;
  noTime: number;
}

interface TimeFixState {
  /** Photos examinées, ou `null` boîte fermée. */
  ids: number[] | null;
  clock: ClockInfo | null;
  /** Horloge du boîtier = UTC + ce décalage ; `null` : le fuseau écrit. */
  offsetMinutes: number | null;
  report: TimeReport | null;
  /** Photos corrigées au dernier passage. */
  corrected: number | null;
  running: boolean;
  error: CommandError | null;
  open: (ids: number[]) => Promise<void>;
  close: () => void;
  setOffset: (minutes: number | null) => Promise<void>;
  apply: () => Promise<void>;
}

export const useTimeFix = create<TimeFixState>()((set, get) => {
  async function examine() {
    const { ids, offsetMinutes } = get();
    if (!ids) return;
    try {
      set({ report: await invoke<TimeReport>("geo_time_proposals", { ids, offsetMinutes }) });
    } catch (e) {
      set({ error: toCommandError(e) });
    }
  }

  return {
    ids: null,
    clock: null,
    offsetMinutes: null,
    report: null,
    corrected: null,
    running: false,
    error: null,

    open: async (ids) => {
      const photos = ids.filter((id) => id > 0);
      if (photos.length === 0) return;
      set({
        ids: photos,
        clock: null,
        offsetMinutes: null,
        report: null,
        corrected: null,
        error: null,
      });
      try {
        const clock = await invoke<ClockInfo>("geo_clock", { ids: photos });
        // Aucun fuseau écrit : celui de l'ordinateur, que la boîte affiche.
        set({
          clock,
          offsetMinutes: clock.writtenMinutes === null ? -new Date().getTimezoneOffset() : null,
        });
      } catch (e) {
        set({ error: toCommandError(e) });
      }
      await examine();
    },

    close: () => set({ ids: null, report: null, corrected: null, error: null }),

    setOffset: async (minutes) => {
      set({ offsetMinutes: minutes, corrected: null });
      await examine();
    },

    apply: async () => {
      const { ids, offsetMinutes } = get();
      if (!ids) return;
      set({ running: true, error: null });
      try {
        const corrected = await invoke<number>("geo_time_correct", { ids, offsetMinutes });
        set({ corrected });
        await examine();
        // Grilles triées par date et fiche de la photo active à jour.
        const library = useLibrary.getState();
        await library.reloadMedia();
        if (library.selectedId !== null) await library.activate(library.selectedId);
      } catch (e) {
        set({ error: toCommandError(e) });
      } finally {
        set({ running: false });
      }
    },
  };
});
