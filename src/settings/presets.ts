import { isTauri } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { api, toCommandError } from "../api/core";

/** Modèles de rangement proposés (cahier des charges §4.1) ; « custom » pour les autres. */
export const FOLDER_PRESETS = [
  { id: "dateAlbum", template: "{yyyy}/{yyyy}-{mm}-{dd} {album}" },
  { id: "yearMonthDay", template: "{yyyy}/{mm}/{dd}" },
  { id: "yearAlbum", template: "{yyyy}/{album}" },
] as const;

/** Renommage : nom du boîtier gardé (par défaut) ou date, heure et numéro. */
export const RENAME_PRESETS = [
  { id: "original", template: null },
  { id: "dateSeq", template: "{yyyy}{mm}{dd}_{hh}{min}{ss}_{camera}_{seq:4}" },
] as const;

export const DEFAULT_FOLDER_TEMPLATE = FOLDER_PRESETS[0].template;

/** « © 2026 Prénom Nom », proposé tant que l'utilisateur ne l'a pas changé. */
export function defaultCopyright(name: string, year = new Date().getFullYear()): string {
  return name.trim() ? `© ${year} ${name.trim()}` : "";
}

/** Chemin d'exemple produit par les modèles, ou l'erreur traduite par son code. */
export function useTemplatePreview(
  folderTemplate: string,
  renameTemplate: string | null,
  album: string,
): { path: string | null; error: string | null } {
  const [result, setResult] = useState<{ path: string | null; error: string | null }>({
    path: null,
    error: null,
  });
  useEffect(() => {
    if (!isTauri()) return;
    let current = true;
    api
      .templatePreview(folderTemplate, renameTemplate, album)
      .then((path) => current && setResult({ path, error: null }))
      .catch((e: unknown) => current && setResult({ path: null, error: toCommandError(e).code }));
    return () => {
      current = false;
    };
  }, [folderTemplate, renameTemplate, album]);
  return result;
}

/** La sauvegarde est-elle sur le disque de la bibliothèque ? */
export function useSameVolume(root: string | null, backup: string | null): boolean {
  const [same, setSame] = useState(false);
  useEffect(() => {
    if (!isTauri() || !root || !backup) return;
    let current = true;
    api
      .sameVolume(root, backup)
      .then((v) => current && setSame(v === true))
      .catch(() => current && setSame(false));
    return () => {
      current = false;
    };
  }, [root, backup]);
  return same && !!root && !!backup;
}
