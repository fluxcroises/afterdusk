import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import { type CommandError, toCommandError } from "../api/core";

/** Miroir de `library::clips::Clip`. */
export interface Clip {
  id: number;
  mediaId: number;
  startMs: number;
  endMs: number;
  name: string;
}

interface ClipsState {
  /** Vidéo dont les extraits sont chargés. */
  mediaId: number | null;
  clips: Clip[];
  /** Images clés du clip (ms) : là où un extrait peut commencer. */
  keyframes: number[];
  /** Export en cours : fait sur total. */
  progress: { done: number; total: number } | null;
  /** Fichiers du dernier export. */
  exported: string[];
  error: CommandError | null;
  load: (mediaId: number) => Promise<void>;
  add: (startMs: number, endMs: number) => Promise<void>;
  rename: (id: number, name: string) => Promise<void>;
  remove: (id: number) => Promise<void>;
  /** Exporte sans réencodage, un fichier par extrait ou réunis. */
  exportClips: (join: boolean) => Promise<void>;
}

/** Image clé au plus tard à `ms` : même calcul que `video::snap`. */
export function snap(keyframes: number[], ms: number): number {
  let out = 0;
  for (const k of keyframes) {
    if (k > ms) break;
    out = k;
  }
  return out;
}

export const useClips = create<ClipsState>()((set, get) => ({
  mediaId: null,
  clips: [],
  keyframes: [],
  progress: null,
  exported: [],
  error: null,

  load: async (mediaId) => {
    set({ mediaId, clips: [], keyframes: [], exported: [], error: null });
    try {
      const [clips, keyframes] = await Promise.all([
        invoke<Clip[]>("clips_list", { mediaId }),
        invoke<number[]>("video_keyframes", { mediaId }).catch(() => []),
      ]);
      if (get().mediaId === mediaId) set({ clips, keyframes });
    } catch (e) {
      set({ error: toCommandError(e) });
    }
  },

  add: async (startMs, endMs) => {
    const { mediaId } = get();
    if (mediaId === null || endMs <= startMs) return;
    try {
      const clip = await invoke<Clip | null>("clips_add", {
        mediaId,
        startMs: Math.round(startMs),
        endMs: Math.round(endMs),
      });
      if (clip && get().mediaId === mediaId) {
        set({ clips: [...get().clips, clip].sort((a, b) => a.startMs - b.startMs) });
      }
    } catch (e) {
      set({ error: toCommandError(e) });
    }
  },

  rename: async (id, name) => {
    try {
      await invoke("clips_rename", { id, name });
      set({ clips: get().clips.map((c) => (c.id === id ? { ...c, name: name.trim() } : c)) });
    } catch (e) {
      set({ error: toCommandError(e) });
    }
  },

  remove: async (id) => {
    try {
      await invoke("clips_delete", { id });
      set({ clips: get().clips.filter((c) => c.id !== id) });
    } catch (e) {
      set({ error: toCommandError(e) });
    }
  },

  exportClips: async (join) => {
    const { mediaId, clips, progress } = get();
    if (mediaId === null || clips.length === 0 || progress) return;
    set({ progress: { done: 0, total: clips.length + (join ? 1 : 0) }, exported: [], error: null });
    const stop = await listen<{ mediaId: number; done: number; total: number }>(
      "clips://progress",
      (e) => {
        if (e.payload.mediaId === mediaId) {
          set({ progress: { done: e.payload.done, total: e.payload.total } });
        }
      },
    ).catch(() => null);
    try {
      set({ exported: await invoke<string[]>("clips_export", { mediaId, join }) });
    } catch (e) {
      set({ error: toCommandError(e) });
    } finally {
      stop?.();
      set({ progress: null });
    }
  },
}));
