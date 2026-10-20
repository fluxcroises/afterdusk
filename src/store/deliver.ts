import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";
import { type CommandError, toCommandError } from "../api/core";

/** Ce que produit un preset d'export, miroir de `export::presets::Output`. */
export type ExportOutput =
  | { kind: "original" }
  | { kind: "jpeg"; quality: number }
  | { kind: "png"; sixteen: boolean }
  | { kind: "tiff"; sixteen: boolean }
  | { kind: "avif"; quality: number };

export type ResizeMode = "full" | "longSide" | "width" | "height" | "percent" | "fit";

export type Resize =
  | { mode: "full" }
  | { mode: "longSide" | "width" | "height"; value: number }
  | { mode: "percent"; value: number }
  | { mode: "fit"; value: [number, number] };

export type ColorSpace = "srgb" | "displayP3" | "adobeRgb";
export type Sharpen = "none" | "screen" | "paper";
export type MetadataPolicy = "all" | "noGps" | "noGpsSerial" | "copyrightOnly" | "none";
export type Anchor =
  | "topLeft"
  | "top"
  | "topRight"
  | "left"
  | "center"
  | "right"
  | "bottomLeft"
  | "bottom"
  | "bottomRight";

export const ANCHORS: Anchor[] = [
  "topLeft",
  "top",
  "topRight",
  "left",
  "center",
  "right",
  "bottomLeft",
  "bottom",
  "bottomRight",
];

export interface ExportPreset {
  id: string;
  name: string | null;
  output: ExportOutput;
  resize: Resize;
  color: ColorSpace;
  sharpen: Sharpen;
  metadata: MetadataPolicy;
  watermark: boolean;
  naming: string;
}

export type WatermarkContent =
  { kind: "text"; text: string; dark: boolean } | { kind: "image"; path: string };

export interface Watermark {
  content: WatermarkContent;
  anchor: Anchor;
  size: number;
  opacity: number;
  margin: number;
}

export interface ExportSettings {
  version: number;
  presets: ExportPreset[];
  watermark: Watermark | null;
  destination: string | null;
  openFolder: boolean;
}

export const BUILTIN_EXPORTS = ["originals", "full", "web", "instagram", "print"];

export interface ExportFailure {
  file: string;
  preset: string;
  code: string;
  detail: string;
}

export interface ExportProgress {
  done: number;
  total: number;
  current: string | null;
  failures: ExportFailure[];
}

export interface ExportFinished {
  progress: ExportProgress | null;
  cancelled: boolean;
  destination: string;
  error: CommandError | null;
}

/** Délai avant d'enregistrer les réglages d'export après une modification. */
export const SAVE_DELAY_MS = 500;

const CHOSEN_KEY = "afterdusk.export.chosen";

function readChosen(): string[] {
  try {
    const raw = window.localStorage.getItem(CHOSEN_KEY);
    const parsed: unknown = raw ? JSON.parse(raw) : null;
    return Array.isArray(parsed) ? parsed.filter((v) => typeof v === "string") : ["web"];
  } catch {
    return ["web"];
  }
}

function writeChosen(ids: string[]) {
  try {
    window.localStorage.setItem(CHOSEN_KEY, JSON.stringify(ids));
  } catch {
    // Préférence de confort : sans stockage, elle vaut pour la session.
  }
}

interface DeliverState {
  settings: ExportSettings | null;
  /** Presets cochés pour le prochain export. */
  chosen: string[];
  /** Preset dont les options sont ouvertes. */
  editing: string | null;
  running: boolean;
  progress: ExportProgress | null;
  finished: ExportFinished | null;
  error: CommandError | null;
  load: () => Promise<void>;
  /** Modifie les réglages (enregistrés peu après). */
  update: (settings: ExportSettings) => void;
  updatePreset: (id: string, patch: Partial<ExportPreset>) => void;
  /** Nouveau preset personnel, copie de `from`. */
  duplicatePreset: (from: string, name: string) => void;
  /** Supprime un preset personnel, ou rétablit un preset fourni. */
  removePreset: (id: string) => Promise<void>;
  toggleChosen: (id: string) => void;
  setEditing: (id: string | null) => void;
  start: (mediaIds: number[], folders: Record<string, string>) => Promise<void>;
  cancel: () => Promise<void>;
  onProgress: (p: ExportProgress) => void;
  onFinished: (f: ExportFinished) => void;
  dismiss: () => void;
}

let saveTimer: ReturnType<typeof setTimeout> | null = null;

export const useDeliver = create<DeliverState>()((set, get) => {
  async function save(settings: ExportSettings): Promise<ExportSettings | null> {
    try {
      return await invoke<ExportSettings>("export_settings_save", { settings });
    } catch (e) {
      set({ error: toCommandError(e) });
      return null;
    }
  }

  function scheduleSave() {
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      const { settings } = get();
      if (settings) void save(settings);
    }, SAVE_DELAY_MS);
  }

  return {
    settings: null,
    chosen: readChosen(),
    editing: null,
    running: false,
    progress: null,
    finished: null,
    error: null,

    load: async () => {
      try {
        const settings = await invoke<ExportSettings>("export_settings");
        const ids = settings.presets.map((p) => p.id);
        set({ settings, chosen: get().chosen.filter((id) => ids.includes(id)) });
      } catch (e) {
        set({ error: toCommandError(e) });
      }
    },

    update: (settings) => {
      set({ settings });
      scheduleSave();
    },

    updatePreset: (id, patch) => {
      const { settings } = get();
      if (!settings) return;
      get().update({
        ...settings,
        presets: settings.presets.map((p) => (p.id === id ? { ...p, ...patch } : p)),
      });
    },

    duplicatePreset: (from, name) => {
      const { settings } = get();
      const source = settings?.presets.find((p) => p.id === from);
      if (!settings || !source) return;
      const next =
        Math.max(0, ...settings.presets.map((p) => Number(/^user-(\d+)$/.exec(p.id)?.[1] ?? 0))) +
        1;
      const id = `user-${next}`;
      get().update({ ...settings, presets: [...settings.presets, { ...source, id, name }] });
      set({ editing: id });
    },

    removePreset: async (id) => {
      const { settings, chosen } = get();
      if (!settings) return;
      if (saveTimer) clearTimeout(saveTimer);
      // Un preset fourni retiré est rétabli par le cœur avec ses valeurs d'origine.
      const saved = await save({
        ...settings,
        presets: settings.presets.filter((p) => p.id !== id),
      });
      if (!saved) return;
      const ids = saved.presets.map((p) => p.id);
      const kept = chosen.filter((c) => ids.includes(c));
      writeChosen(kept);
      set({ settings: saved, chosen: kept, editing: ids.includes(id) ? id : null });
    },

    toggleChosen: (id) => {
      const { chosen } = get();
      const next = chosen.includes(id) ? chosen.filter((c) => c !== id) : [...chosen, id];
      writeChosen(next);
      set({ chosen: next });
    },

    setEditing: (id) => set({ editing: get().editing === id ? null : id }),

    start: async (mediaIds, folders) => {
      const { settings, chosen, running } = get();
      if (!settings?.destination || running || mediaIds.length === 0) return;
      if (saveTimer) {
        clearTimeout(saveTimer);
        saveTimer = null;
        if (!(await save(settings))) return;
      }
      const targets = settings.presets
        .filter((p) => chosen.includes(p.id))
        .map((p) => ({ preset: p.id, folder: folders[p.id] ?? p.id }));
      if (targets.length === 0) return;
      set({
        running: true,
        finished: null,
        error: null,
        progress: { done: 0, total: mediaIds.length * targets.length, current: null, failures: [] },
      });
      try {
        await invoke("export_start", {
          request: { mediaIds, targets, destination: settings.destination },
        });
      } catch (e) {
        set({ running: false, progress: null, error: toCommandError(e) });
      }
    },

    cancel: async () => {
      try {
        await invoke("export_cancel");
      } catch (e) {
        set({ error: toCommandError(e) });
      }
    },

    onProgress: (progress) => set({ progress }),

    onFinished: (finished) =>
      set({
        running: false,
        finished,
        progress: finished.progress ?? get().progress,
        error: finished.error,
      }),

    dismiss: () => set({ finished: null, progress: null }),
  };
});
