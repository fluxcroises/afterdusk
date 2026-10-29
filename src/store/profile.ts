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
  /** Lit le profil enregistré ; faux si le cœur n'a pas répondu. */
  load: () => Promise<boolean>;
  set: (profile: Profile) => Promise<void>;
  setRevealed: (revealed: boolean) => void;
}

export const useProfile = create<ProfileState>()((set) => ({
  profile: "complete",
  revealed: false,
  error: null,

  load: async () => {
    try {
      set({ profile: await invoke<Profile>("interface_profile_get"), error: null });
      return true;
    } catch (e) {
      set({ error: toCommandError(e) });
      return false;
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

/** Lectures du profil au lancement, et attente entre deux. */
const PROFILE_ATTEMPTS = 40;
const PROFILE_RETRY_MS = 250;

/** Suit les changements faits dans une autre fenêtre (accueil, Réglages). */
export async function watchProfile(): Promise<void> {
  // Un module masqué ne reste pas affiché : retour à Importer.
  useProfile.subscribe((s) => {
    const { screen, goTo } = useNavigation.getState();
    if (!allows(s.profile, `module.${screen}`)) goTo("import");
  });
  await listen<Profile>("profile://changed", (e) =>
    useProfile.setState({ profile: e.payload }),
  ).catch(() => undefined);
  // Au lancement, les fenêtres se chargent pendant que le cœur ouvre la base :
  // une lecture trop tôt échoue. Nouvel essai, jusqu'à dix secondes, sans quoi
  // l'interface resterait complète quel que soit le profil enregistré.
  for (let attempt = 0; attempt < PROFILE_ATTEMPTS; attempt++) {
    if (await useProfile.getState().load()) return;
    await new Promise((resolve) => setTimeout(resolve, PROFILE_RETRY_MS));
  }
}
