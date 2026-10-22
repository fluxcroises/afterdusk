import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";
import { type CommandError, toCommandError } from "../api/core";
import type { SheetWatermark } from "./contactSheet";

export type GalleryTheme = "light" | "dark";

/** Options gardées d'une galerie à l'autre. */
export interface GalleryOptions {
  theme: GalleryTheme;
  allowDownload: boolean;
  selection: boolean;
  watermark: SheetWatermark;
}

/** Miroir de `export::gallery::GalleryRequest`. */
export interface GalleryRequest extends GalleryOptions {
  libraryId: number;
  title: string;
  subtitle: string;
  intro: string;
  albumId: number | null;
  clientId: number | null;
  mediaIds: number[];
  destination: string;
  texts: Record<string, string>;
}

export interface GalleryResult {
  code: string;
  folder: string;
  photos: number;
  missing: number;
}

/** Miroir de `export::gallery::SelectionMatch`. */
export interface SelectionMatch {
  galleryId: number;
  libraryId: number;
  code: string;
  title: string;
  clientId: number | null;
  clientName: string | null;
  albumId: number | null;
  mediaIds: number[];
  unknown: number[];
}

const OPTIONS_KEY = "afterdusk.gallery";

export const DEFAULT_GALLERY_OPTIONS: GalleryOptions = {
  theme: "light",
  allowDownload: false,
  selection: true,
  watermark: { kind: "none" },
};

function readOptions(): GalleryOptions {
  try {
    const raw = window.localStorage.getItem(OPTIONS_KEY);
    return raw
      ? { ...DEFAULT_GALLERY_OPTIONS, ...(JSON.parse(raw) as Partial<GalleryOptions>) }
      : DEFAULT_GALLERY_OPTIONS;
  } catch {
    return DEFAULT_GALLERY_OPTIONS;
  }
}

interface GalleryState {
  open: boolean;
  options: GalleryOptions;
  running: boolean;
  progress: { done: number; total: number } | null;
  result: GalleryResult | null;
  error: CommandError | null;
  show: () => void;
  close: () => void;
  setOptions: (patch: Partial<GalleryOptions>) => void;
  onProgress: (progress: { done: number; total: number }) => void;
  run: (request: GalleryRequest) => Promise<void>;

  /** Boîte « Sélection client » : texte collé ou lu, galerie retrouvée. */
  importOpen: boolean;
  text: string;
  found: SelectionMatch | null;
  importError: CommandError | null;
  showImport: () => void;
  closeImport: () => void;
  read: (text: string) => Promise<void>;
  readFile: (path: string) => Promise<void>;
  /** Pose « Retenue » et crée la collection ; renvoie son identifiant. */
  apply: (collectionName: string) => Promise<number | null>;
}

export const useGallery = create<GalleryState>()((set, get) => ({
  open: false,
  options: readOptions(),
  running: false,
  progress: null,
  result: null,
  error: null,

  show: () => set({ open: true, error: null, result: null }),
  close: () => {
    if (!get().running) set({ open: false });
  },

  setOptions: (patch) => {
    const options = { ...get().options, ...patch };
    set({ options });
    try {
      window.localStorage.setItem(OPTIONS_KEY, JSON.stringify(options));
    } catch {
      // Préférence de confort : sans stockage, elle vaut pour la session.
    }
  },

  onProgress: (progress) => set({ progress }),

  run: async (request) => {
    set({ running: true, progress: null, error: null, result: null });
    try {
      const result = await invoke<GalleryResult>("gallery_create", { request });
      set({ result });
    } catch (e) {
      set({ error: toCommandError(e) });
    } finally {
      set({ running: false, progress: null });
    }
  },

  importOpen: false,
  text: "",
  found: null,
  importError: null,

  showImport: () => set({ importOpen: true, text: "", found: null, importError: null }),
  closeImport: () => set({ importOpen: false }),

  read: async (text) => {
    set({ text, found: null, importError: null });
    if (!text.trim()) return;
    try {
      set({ found: await invoke<SelectionMatch>("gallery_read_selection", { text }) });
    } catch (e) {
      set({ importError: toCommandError(e) });
    }
  },

  readFile: async (path) => {
    try {
      await get().read(await invoke<string>("gallery_selection_file", { path }));
    } catch (e) {
      set({ importError: toCommandError(e) });
    }
  },

  apply: async (collectionName) => {
    try {
      const applied = await invoke<{ flagged: number; collectionId: number }>(
        "gallery_apply_selection",
        { text: get().text, collectionName },
      );
      set({ importOpen: false });
      return applied.collectionId;
    } catch (e) {
      set({ importError: toCommandError(e) });
      return null;
    }
  },
}));
