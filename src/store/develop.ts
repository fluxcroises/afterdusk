import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";
import { type CommandError, toCommandError } from "../api/core";

/** Réglages de retouche, miroir de `develop::settings::DevelopSettings` (JSON creux). */
export interface DevelopSettings {
  version: number;
  process: number;
  light?: {
    exposure?: number;
    contrast?: number;
    highlights?: number;
    shadows?: number;
    whites?: number;
    blacks?: number;
  };
  color?: {
    whiteBalance?:
      | { mode: "asShot" }
      | { mode: "preset"; preset: string }
      | { mode: "custom"; temperature: number; tint: number };
    vibrance?: number;
    saturation?: number;
  };
}

interface DevelopOpened {
  mediaId: number;
  width: number;
  height: number;
  settings: DevelopSettings;
  adapter: string | null;
  decodeMs: number;
}

/** Rendu reçu du cœur : en-tête de 16 octets puis pixels RGBA. */
export interface DevelopFrame {
  width: number;
  height: number;
  /** Durée du rendu dans le cœur, en millisecondes. */
  renderMs: number;
  gpu: boolean;
  pixels: Uint8ClampedArray<ArrayBuffer>;
}

export function parseFrame(buffer: ArrayBuffer): DevelopFrame {
  const header = new DataView(buffer, 0, 16);
  const width = header.getUint32(0, true);
  const height = header.getUint32(4, true);
  return {
    width,
    height,
    renderMs: header.getUint32(8, true) / 1000,
    gpu: header.getUint32(12, true) === 1,
    pixels: new Uint8ClampedArray(buffer, 16, width * height * 4),
  };
}

/** Plage du curseur d'exposition, en IL (décision 0005). */
export const EXPOSURE_RANGE = { min: -5, max: 5, step: 0.05 } as const;

/** Délai après le dernier geste avant l'enregistrement (historique et XMP). */
export const SAVE_DELAY_MS = 600;

interface DevelopState {
  mediaId: number | null;
  loading: boolean;
  error: CommandError | null;
  settings: DevelopSettings | null;
  adapter: string | null;
  decodeMs: number | null;
  frame: DevelopFrame | null;
  /** Temps entre le geste et l'image reçue, interface comprise. */
  roundTripMs: number | null;
  open: (mediaId: number) => Promise<void>;
  setExposure: (value: number) => void;
  /** Enregistre tout de suite un réglage en attente (changement de photo, sortie). */
  flush: () => Promise<void>;
  close: () => Promise<void>;
}

let openToken = 0;
let rendering = false;
let renderAgain = false;
let saveTimer: ReturnType<typeof setTimeout> | null = null;
let pendingLabel: string | null = null;

export const useDevelop = create<DevelopState>()((set, get) => {
  /** Un seul rendu à la fois ; les gestes reçus entre-temps n'en déclenchent qu'un. */
  async function render() {
    if (rendering) {
      renderAgain = true;
      return;
    }
    rendering = true;
    try {
      do {
        renderAgain = false;
        const { settings, mediaId } = get();
        if (!settings || mediaId === null) return;
        const started = performance.now();
        const buffer = await invoke<ArrayBuffer>("develop_render", { settings });
        if (get().mediaId !== mediaId) return;
        set({ frame: parseFrame(buffer), roundTripMs: performance.now() - started });
      } while (renderAgain);
    } catch (e) {
      set({ error: toCommandError(e) });
    } finally {
      rendering = false;
    }
  }

  function scheduleSave(label: string) {
    pendingLabel = label;
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => void get().flush(), SAVE_DELAY_MS);
  }

  return {
    mediaId: null,
    loading: false,
    error: null,
    settings: null,
    adapter: null,
    decodeMs: null,
    frame: null,
    roundTripMs: null,

    open: async (mediaId) => {
      if (get().mediaId === mediaId && get().settings) return;
      await get().flush();
      const token = ++openToken;
      set({ mediaId, loading: true, error: null, settings: null, frame: null });
      try {
        const opened = await invoke<DevelopOpened>("develop_open", { mediaId });
        if (token !== openToken) return;
        set({
          loading: false,
          settings: opened.settings,
          adapter: opened.adapter,
          decodeMs: opened.decodeMs,
        });
        await render();
      } catch (e) {
        if (token === openToken) set({ loading: false, error: toCommandError(e) });
      }
    },

    setExposure: (value) => {
      const { settings } = get();
      if (!settings) return;
      const exposure = Math.round(value * 100) / 100;
      const light = { ...settings.light };
      if (exposure === 0) delete light.exposure;
      else light.exposure = exposure;
      set({ settings: { ...settings, light } });
      void render();
      scheduleSave("light.exposure");
    },

    flush: async () => {
      if (saveTimer) clearTimeout(saveTimer);
      saveTimer = null;
      const label = pendingLabel;
      pendingLabel = null;
      const { settings, mediaId } = get();
      if (!label || !settings || mediaId === null) return;
      try {
        await invoke("develop_save", { mediaId, settings, label });
      } catch (e) {
        set({ error: toCommandError(e) });
      }
    },

    close: async () => {
      await get().flush();
      openToken++;
      set({ mediaId: null, settings: null, frame: null, loading: false, error: null });
      await invoke("develop_close").catch(() => undefined);
    },
  };
});
