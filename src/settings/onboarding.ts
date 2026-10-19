import { isTauri } from "@tauri-apps/api/core";
import { type Identity, api } from "../api/core";

export interface OnboardingChoices {
  name: string;
  root: string | null;
  backup: string | null;
  folderTemplate: string;
  renameTemplate: string | null;
  identity: Identity;
}

/** Dossier proposé : Images/Afterdusk (créé à la fin de l'assistant). */
export async function defaultRoot(): Promise<string | null> {
  if (!isTauri()) return null;
  try {
    const { join, pictureDir } = await import("@tauri-apps/api/path");
    return await join(await pictureDir(), "Afterdusk");
  } catch {
    return null;
  }
}

/** Crée la bibliothèque et enregistre les choix, dans cet ordre. */
export async function applyChoices(c: OnboardingChoices): Promise<void> {
  if (!c.root) return;
  const library = await api.libraryCreate(c.name.trim() || c.root, c.root, c.backup);
  await api.librarySetTemplates(library.id, c.folderTemplate, c.renameTemplate);
  await api.identitySet(c.identity);
}
