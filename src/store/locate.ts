import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";
import { type CommandError, toCommandError } from "../api/core";
import { useImport } from "./import";
import { useLibrary } from "./library";

/** Bilan d'une localisation, miroir de `geo::locate::LocateReport`. */
export interface LocateReport {
  located: number;
  kept: number;
  unmatched: number;
  noTime: number;
  unusableSources: number;
}

/** Fuseau écrit par les boîtiers dans la sélection. */
export interface ClockInfo {
  writtenMinutes: number | null;
  /** Fuseaux différents d'une photo à l'autre. */
  mixed: boolean;
  withOffset: number;
  total: number;
}

export type LocateSource = "gpx" | "phone" | "manual";

/** Lieu trouvé par son nom, miroir de `geo::places::PlaceName`. */
export interface PlaceName {
  city: string;
  region: string | null;
  country: string;
  countryName: string;
  timezone: string;
  latitude: number;
  longitude: number;
}

/** Lieux dont le nom commence par `query`, les plus peuplés d'abord. */
export function searchPlaces(query: string): Promise<PlaceName[]> {
  return invoke<PlaceName[]>("geo_search_places", { query });
}

export interface LocateRequest {
  source: LocateSource;
  files: string[];
  /** Saisie manuelle : le lieu choisi. */
  place: { latitude: number; longitude: number } | null;
  /** Heure du boîtier = UTC + ce décalage ; `null` : le fuseau écrit. */
  offsetMinutes: number | null;
  /** Écart maximal sur la trace, ou fenêtre autour des photos de téléphone. */
  minutes: number;
  replace: boolean;
}

interface LocateState {
  /** Photos à localiser, ou `null` boîte fermée. */
  ids: number[] | null;
  clock: ClockInfo | null;
  running: boolean;
  report: LocateReport | null;
  error: CommandError | null;
  open: (ids: number[]) => Promise<void>;
  close: () => void;
  run: (request: LocateRequest) => Promise<void>;
}

export const useLocate = create<LocateState>()((set, get) => ({
  ids: null,
  clock: null,
  running: false,
  report: null,
  error: null,

  open: async (ids) => {
    const photos = ids.filter((id) => id > 0);
    if (photos.length === 0) return;
    set({ ids: photos, clock: null, report: null, error: null });
    try {
      set({ clock: await invoke<ClockInfo>("geo_clock", { ids: photos }) });
    } catch (e) {
      set({ error: toCommandError(e) });
    }
  },

  close: () => set({ ids: null, report: null, error: null }),

  run: async ({ source, files, place, offsetMinutes, minutes, replace }) => {
    const { ids } = get();
    if (!ids) return;
    set({ running: true, error: null, report: null });
    try {
      const report =
        source === "manual"
          ? await invoke<LocateReport>("geo_locate_manual", {
              ids,
              latitude: place?.latitude,
              longitude: place?.longitude,
              replace,
            })
          : source === "gpx"
            ? await invoke<LocateReport>("geo_locate_gpx", {
                ids,
                files,
                offsetMinutes,
                maxGapMinutes: minutes,
                replace,
              })
            : await invoke<LocateReport>("geo_locate_phone", {
                ids,
                files,
                offsetMinutes,
                windowMinutes: minutes,
                replace,
              });
      set({ report });
      // La proposition de fin d'import ne compte plus les photos localisées.
      void useImport.getState().refreshUnlocated();
      // La fiche de la photo active montre le nouveau lieu ; la sélection reste.
      const library = useLibrary.getState();
      if (library.selectedId !== null) await library.activate(library.selectedId);
    } catch (e) {
      set({ error: toCommandError(e) });
    } finally {
      set({ running: false });
    }
  },
}));
