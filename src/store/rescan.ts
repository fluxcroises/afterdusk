import { invoke, isTauri } from "@tauri-apps/api/core";
import i18next from "i18next";
import { create } from "zustand";
import { toCommandError } from "../api/core";

export interface RescanReport {
  added: number;
  restored: number;
  failed: [string, string][];
}

interface RescanState {
  running: boolean;
  progress: { done: number; total: number } | null;
  /** Réanalyse la bibliothèque, puis en donne le bilan. */
  run: (libraryId: number) => Promise<void>;
  onProgress: (p: { done: number; total: number }) => void;
}

/** Bilan affiché à la fin (boîte native, ou alerte du navigateur en développement). */
async function tell(text: string) {
  if (!isTauri()) {
    window.alert(text);
    return;
  }
  const { message } = await import("@tauri-apps/plugin-dialog");
  await message(text, { title: i18next.t("rescan.title"), kind: "info" });
}

export const useRescan = create<RescanState>()((set, get) => ({
  running: false,
  progress: null,
  run: async (libraryId) => {
    if (get().running) return;
    set({ running: true, progress: null });
    try {
      const r = await invoke<RescanReport>("library_rescan", { libraryId });
      const lines = [
        i18next.t("rescan.added", { count: r.added }),
        i18next.t("rescan.restored", { count: r.restored }),
      ];
      if (r.failed.length > 0) {
        lines.push(i18next.t("rescan.failed", { count: r.failed.length }));
        lines.push(...r.failed.slice(0, 5).map(([path, why]) => `${path} : ${why}`));
      }
      await tell(lines.join("\n"));
    } catch (e) {
      const error = toCommandError(e);
      await tell(i18next.t(`errors.${error.code}`, { defaultValue: i18next.t("errors.internal") }));
    } finally {
      set({ running: false, progress: null });
    }
  },
  onProgress: (progress) => set({ progress }),
}));
