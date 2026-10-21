import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";
import { type CommandError, toCommandError } from "../api/core";
import type { Area } from "../ui/geo";
import { useLibrary } from "./library";

/** Album sur la carte, miroir de `geo::map::AlbumPoint`. */
export interface AlbumPoint {
  albumId: number;
  name: string;
  startDate: string | null;
  endDate: string | null;
  latitude: number;
  longitude: number;
  located: number;
  total: number;
  coverMediaId: number;
  city: string | null;
  country: string | null;
}

export interface MediaPoint {
  id: number;
  albumId: number;
  latitude: number;
  longitude: number;
}

/** Outil actif : déplacer la carte, tracer une zone, placer la sélection. */
export type MapMode = "browse" | "area" | "place";

const ONLINE_KEY = "afterdusk.map.online";

/** Fond de carte en ligne (OpenFreeMap) : activé sauf si l'utilisateur l'a coupé. */
function readOnline(): boolean {
  try {
    return window.localStorage.getItem(ONLINE_KEY) !== "false";
  } catch {
    return true;
  }
}

interface MapState {
  online: boolean;
  albums: AlbumPoint[];
  media: MediaPoint[];
  loaded: boolean;
  selectedAlbumId: number | null;
  /** Traces GPX des jours de l'album choisi, en lignes [longitude, latitude]. */
  tracks: number[][][];
  mode: MapMode;
  area: Area | null;
  /** Photos placées au dernier clic en mode « placer ». */
  placed: number | null;
  error: CommandError | null;
  setOnline: (online: boolean) => void;
  load: (libraryId: number) => Promise<void>;
  selectAlbum: (albumId: number | null) => Promise<void>;
  setMode: (mode: MapMode) => void;
  setArea: (area: Area | null) => void;
  /** Pose le lieu cliqué sur les photos sélectionnées (sans remplacer). */
  place: (latitude: number, longitude: number) => Promise<void>;
}

export const useMap = create<MapState>()((set, get) => ({
  online: readOnline(),
  albums: [],
  media: [],
  loaded: false,
  selectedAlbumId: null,
  tracks: [],
  mode: "browse",
  area: null,
  placed: null,
  error: null,

  setOnline: (online) => {
    try {
      window.localStorage.setItem(ONLINE_KEY, String(online));
    } catch {
      // Stockage indisponible : le choix vaut pour cette session.
    }
    set({ online });
  },

  load: async (libraryId) => {
    try {
      const data = await invoke<{ albums: AlbumPoint[]; media: MediaPoint[] }>("geo_map", {
        libraryId,
      });
      const selected = get().selectedAlbumId;
      set({
        albums: data.albums,
        media: data.media,
        loaded: true,
        selectedAlbumId: data.albums.some((a) => a.albumId === selected) ? selected : null,
      });
    } catch (e) {
      set({ error: toCommandError(e), loaded: true });
    }
  },

  selectAlbum: async (albumId) => {
    set({ selectedAlbumId: albumId, tracks: [] });
    const album = get().albums.find((a) => a.albumId === albumId);
    const libraryId = useLibrary.getState().libraryId;
    if (!album?.startDate || libraryId === null) return;
    try {
      const tracks = await invoke<number[][][]>("geo_album_tracks", {
        libraryId,
        startDate: album.startDate,
        endDate: album.endDate ?? album.startDate,
      });
      if (get().selectedAlbumId === albumId) set({ tracks });
    } catch (e) {
      set({ error: toCommandError(e) });
    }
  },

  setMode: (mode) => set({ mode: get().mode === mode ? "browse" : mode, placed: null }),

  setArea: (area) => set({ area, mode: "browse" }),

  place: async (latitude, longitude) => {
    const { selectedIds, selectedId, libraryId } = useLibrary.getState();
    const ids = (
      selectedIds.length > 0 ? selectedIds : selectedId !== null ? [selectedId] : []
    ).filter((id) => id > 0);
    if (ids.length === 0 || libraryId === null) return;
    try {
      const report = await invoke<{ located: number }>("geo_locate_manual", {
        ids,
        latitude,
        longitude,
        replace: false,
      });
      set({ placed: report.located, mode: "browse" });
      await get().load(libraryId);
    } catch (e) {
      set({ error: toCommandError(e) });
    }
  },
}));
