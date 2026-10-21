import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";
import { type CommandError, toCommandError } from "../api/core";

export type Paper = "a4" | "letter";
export type Orientation = "portrait" | "landscape";
export type SheetWatermark = { kind: "none" } | { kind: "text"; text: string } | { kind: "logo" };

/** Options de mise en page, gardées d'une planche à l'autre. */
export interface SheetOptions {
  columns: number;
  paper: Paper;
  orientation: Orientation;
  headerLogo: boolean;
  watermark: SheetWatermark;
}

/** Miroir de `export::contact_sheet::ContactSheet`. */
export interface ContactSheet extends SheetOptions {
  title: string;
  subtitle: string;
  footer: string;
  items: { mediaId: number; caption: string }[];
}

const OPTIONS_KEY = "afterdusk.contactSheet";

export const DEFAULT_OPTIONS: SheetOptions = {
  columns: 4,
  paper: "a4",
  orientation: "portrait",
  headerLogo: true,
  watermark: { kind: "none" },
};

function readOptions(): SheetOptions {
  try {
    const raw = window.localStorage.getItem(OPTIONS_KEY);
    return raw
      ? { ...DEFAULT_OPTIONS, ...(JSON.parse(raw) as Partial<SheetOptions>) }
      : DEFAULT_OPTIONS;
  } catch {
    return DEFAULT_OPTIONS;
  }
}

interface ContactSheetState {
  open: boolean;
  options: SheetOptions;
  running: boolean;
  progress: { done: number; total: number } | null;
  error: CommandError | null;
  show: () => void;
  close: () => void;
  setOptions: (patch: Partial<SheetOptions>) => void;
  onProgress: (progress: { done: number; total: number }) => void;
  /** Écrit la planche à `path` ; ferme la boîte une fois le fichier écrit. */
  run: (path: string, sheet: ContactSheet) => Promise<void>;
}

export const useContactSheet = create<ContactSheetState>()((set, get) => ({
  open: false,
  options: readOptions(),
  running: false,
  progress: null,
  error: null,

  show: () => set({ open: true, error: null }),
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

  run: async (path, sheet) => {
    set({ running: true, progress: null, error: null });
    try {
      await invoke("contact_sheet_export", { path, sheet });
      set({ open: false });
    } catch (e) {
      set({ error: toCommandError(e) });
    } finally {
      set({ running: false, progress: null });
    }
  },
}));
