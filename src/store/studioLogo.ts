import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";
import { type CommandError, toCommandError } from "../api/core";

interface StudioLogoState {
  /** Aperçu du logo (adresse `blob:`) ; `null` sans logo. */
  url: string | null;
  error: CommandError | null;
  load: () => Promise<void>;
  /** Copie le fichier choisi comme logo du studio. */
  set: (path: string) => Promise<void>;
  clear: () => Promise<void>;
}

/** Logo du studio (Réglages > Identité) : documents PDF et filigranes. */
export const useStudioLogo = create<StudioLogoState>()((set, get) => {
  async function refresh() {
    const previous = get().url;
    if (previous) URL.revokeObjectURL(previous);
    const bytes = await invoke<ArrayBuffer | null>("identity_logo");
    set({
      url: bytes && bytes.byteLength > 0 ? URL.createObjectURL(new Blob([bytes])) : null,
    });
  }

  async function guard(run: () => Promise<void>) {
    try {
      await run();
      set({ error: null });
    } catch (e) {
      set({ error: toCommandError(e) });
    }
  }

  return {
    url: null,
    error: null,
    load: () => guard(refresh),
    set: (path) =>
      guard(async () => {
        await invoke("identity_logo_set", { path });
        await refresh();
      }),
    clear: () =>
      guard(async () => {
        await invoke("identity_logo_clear");
        await refresh();
      }),
  };
});
