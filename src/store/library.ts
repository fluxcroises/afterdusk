import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { type CommandError, toCommandError } from "../api/core";

export interface MediaSummary {
  id: number;
  albumId: number;
  fileName: string;
  capturedAt: string | null;
  rating: number;
  flag: number;
  color: string | null;
  width: number | null;
  height: number | null;
  orientation: number;
  hasPreview: boolean;
}

export interface AlbumSummary {
  id: number;
  name: string;
  folder: string;
  startDate: string | null;
  endDate: string | null;
  count: number;
}

export interface MediaDetails {
  id: number;
  fileName: string;
  originalName: string;
  path: string;
  capturedAt: string | null;
  utcOffset: string | null;
  width: number | null;
  height: number | null;
  camera: string | null;
  lens: string | null;
  focalLength: number | null;
  focalLength35mm: number | null;
  aperture: number | null;
  shutter: number | null;
  iso: number | null;
  exposureCompensation: number | null;
  keywords: string[];
}

export const libraryApi = {
  mediaList: (libraryId: number, albumId: number | null) =>
    invoke<MediaSummary[]>("media_list", { libraryId, albumId }),
  albumsList: (libraryId: number) => invoke<AlbumSummary[]>("albums_list", { libraryId }),
  mediaDetails: (mediaId: number) => invoke<MediaDetails | null>("media_details", { mediaId }),
};

interface LibraryState {
  libraryId: number | null;
  albums: AlbumSummary[];
  /** Album affiché ; `null` pour toutes les photos. */
  albumId: number | null;
  media: MediaSummary[];
  /** Nombre total de photos de la bibliothèque. */
  total: number;
  selectedId: number | null;
  details: MediaDetails | null;
  /** Incrémenté à chaque aperçu régénéré, pour rafraîchir l'image. */
  versions: Record<number, number>;
  error: CommandError | null;

  load: (libraryId: number | null) => Promise<void>;
  showAlbum: (albumId: number | null) => Promise<void>;
  select: (mediaId: number) => Promise<void>;
  move: (delta: number) => void;
  previewReady: (mediaId: number) => void;
}

export const useLibrary = create<LibraryState>((set, get) => ({
  libraryId: null,
  albums: [],
  albumId: null,
  media: [],
  total: 0,
  selectedId: null,
  details: null,
  versions: {},
  error: null,

  load: async (libraryId) => {
    if (libraryId === null) {
      set({ libraryId, albums: [], media: [], total: 0, selectedId: null, details: null });
      return;
    }
    try {
      const albumId = get().libraryId === libraryId ? get().albumId : null;
      const [albums, media] = await Promise.all([
        libraryApi.albumsList(libraryId),
        libraryApi.mediaList(libraryId, albumId),
      ]);
      const total = albums.reduce((n, a) => n + a.count, 0);
      const selectedId = media.some((m) => m.id === get().selectedId)
        ? get().selectedId
        : (media[0]?.id ?? null);
      set({ libraryId, albums, albumId, media, total, error: null });
      if (selectedId !== null) await get().select(selectedId);
      else set({ selectedId: null, details: null });
    } catch (e) {
      set({ error: toCommandError(e) });
    }
  },

  showAlbum: async (albumId) => {
    const { libraryId } = get();
    if (libraryId === null) return;
    try {
      const media = await libraryApi.mediaList(libraryId, albumId);
      set({ albumId, media });
      const first = media[0];
      if (first) await get().select(first.id);
      else set({ selectedId: null, details: null });
    } catch (e) {
      set({ error: toCommandError(e) });
    }
  },

  select: async (mediaId) => {
    set({ selectedId: mediaId });
    try {
      const details = await libraryApi.mediaDetails(mediaId);
      // Une sélection plus récente a pu arriver pendant la requête.
      if (get().selectedId === mediaId) set({ details });
    } catch (e) {
      set({ error: toCommandError(e) });
    }
  },

  move: (delta) => {
    const { media, selectedId } = get();
    if (media.length === 0) return;
    const index = media.findIndex((m) => m.id === selectedId);
    const next = media[Math.min(Math.max(index + delta, 0), media.length - 1)];
    if (next && next.id !== selectedId) void get().select(next.id);
  },

  previewReady: (mediaId) =>
    set((s) => ({
      versions: { ...s.versions, [mediaId]: (s.versions[mediaId] ?? 0) + 1 },
      media: s.media.map((m) => (m.id === mediaId ? { ...m, hasPreview: true } : m)),
    })),
}));
