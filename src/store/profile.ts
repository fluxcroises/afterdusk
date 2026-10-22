import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import { type CommandError, toCommandError } from "../api/core";
import { type Profile, allows } from "../ui/profiles";
import { useNavigation } from "./navigation";

interface ProfileState {
  /** Complète tant que rien n'est lu : rien n'est masqué par erreur. */
  profile: Profile;
  /** Sections avancées montrées pour la session (« Afficher les réglages avancés »). */
  revealed: boolean;
  error: CommandError | null;
  load: () => Promise<void>;
  set: (profile: Profile) => Promise<void>;
  setRevealed: (revealed: boolean) => void;
}

export const useProfile = create<ProfileState>()((set) => ({
  profile: "complete",
  revealed: false,
  error: null,

  load: async () => {
    try {
      set({ profile: await invoke<Profile>("interface_profile_get") });
    } catch (e) {
      set({ error: toCommandError(e) });
    }
  },

  set: async (profile) => {
    try {
      await invoke("interface_profile_set", { profile });
      set({ profile, revealed: false });
    } catch (e) {
      set({ error: toCommandError(e) });
    }
  },

  setRevealed: (revealed) => set({ revealed }),
}));

/** Le profil affiche-t-il cette fonction ? */
export function useAllows(feature: string): boolean {
  return useProfile((s) => allows(s.profile, feature));
}

/** Pour le code hors composants (menus, raccourcis). */
export function profileAllows(feature: string): boolean {
  return allows(useProfile.getState().profile, feature);
}

/** Suit les changements faits dans une autre fenêtre (accueil, Réglages). */
export async function watchProfile(): Promise<void> {
  // Un module masqué ne reste pas affiché : retour à Importer.
  useProfile.subscribe((s) => {
    const { screen, goTo } = useNavigation.getState();
    if (!allows(s.profile, `module.${screen}`)) goTo("import");
  });
  await useProfile.getState().load();
  await listen<Profile>("profile://changed", (e) =>
    useProfile.setState({ profile: e.payload }),
  ).catch(() => undefined);
}
