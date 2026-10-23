import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import { type CommandError, toCommandError } from "../api/core";

/** Mise à jour trouvée par la vérification du démarrage (décision 0013). */
export interface UpdateInfo {
  version: string;
  notes: string | null;
}

export interface UpdateProgress {
  downloaded: number;
  total: number | null;
}

interface UpdatesState {
  /** Vérification activée dans Réglages > Général ; null tant qu'elle n'est pas lue. */
  enabled: boolean | null;
  available: UpdateInfo | null;
  dismissed: boolean;
  progress: UpdateProgress | null;
  error: CommandError | null;
  loadEnabled: () => Promise<void>;
  setEnabled: (enabled: boolean) => Promise<void>;
  /** Au démarrage : le cœur ne fait la requête que si elle est due. */
  check: () => Promise<void>;
  install: () => Promise<void>;
  dismiss: () => void;
}

export const useUpdates = create<UpdatesState>()((set, get) => ({
  enabled: null,
  available: null,
  dismissed: false,
  progress: null,
  error: null,

  loadEnabled: async () => {
    if (!isTauri()) return;
    try {
      set({ enabled: await invoke<boolean>("updates_enabled_get") });
    } catch (e) {
      set({ error: toCommandError(e) });
    }
  },

  setEnabled: async (enabled) => {
    set({ enabled });
    try {
      await invoke("updates_enabled_set", { enabled });
    } catch (e) {
      set({ error: toCommandError(e) });
    }
  },

  check: async () => {
    if (!isTauri()) return;
    try {
      const available = await invoke<UpdateInfo | null>("update_check");
      set({ available, dismissed: false });
    } catch {
      // Sans réponse, rien à proposer : la vérification reprendra le lendemain.
    }
  },

  install: async () => {
    if (get().progress) return;
    set({ progress: { downloaded: 0, total: null }, error: null });
    const unlisten = await listen<UpdateProgress>("update://progress", (e) =>
      set({ progress: e.payload }),
    );
    try {
      // L'application redémarre une fois la mise à jour installée.
      await invoke("update_install");
    } catch (e) {
      set({ progress: null, error: toCommandError(e) });
    } finally {
      unlisten();
    }
  },

  dismiss: () => set({ dismissed: true }),
}));
