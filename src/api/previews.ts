import { convertFileSrc, isTauri } from "@tauri-apps/api/core";

export type PreviewSize = 256 | 1024 | 2048;

/** Base du protocole `preview://` (`http://preview.localhost/` sous Windows). */
function base(): string | null {
  return isTauri() ? convertFileSrc("", "preview") : null;
}

/**
 * Aperçu en cache d'un média. `version` change quand l'aperçu est régénéré,
 * pour que la vue web ne garde pas une ancienne image.
 */
export function previewUrl(mediaId: number, size: PreviewSize, version = 0): string | null {
  const root = base();
  return root ? `${root}media/${mediaId}/${size}?v=${version}` : null;
}

/**
 * JPEG pleine définition (le plus grand aperçu intégré au RAW), pour le zoom
 * 100 %. L'orientation n'y est pas appliquée : l'interface la fait elle-même.
 */
export function fullUrl(mediaId: number): string | null {
  const root = base();
  return root ? `${root}media/${mediaId}/full` : null;
}

/** Miniature d'un fichier de la source analysée, avant import. */
export function sourceThumbnailUrl(path: string): string | null {
  const root = base();
  return root ? `${root}source?path=${encodeURIComponent(path)}` : null;
}
