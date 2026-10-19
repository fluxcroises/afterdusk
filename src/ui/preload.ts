import { previewUrl } from "../api/previews";
import type { MediaSummary } from "../store/library";

/** Aperçu affiché par la visionneuse et la comparaison. */
export const displaySrc = (m: MediaSummary, version: number) =>
  m.hasPreview ? previewUrl(m.id, 2048, version) : null;

/** Images décodées d'avance, gardées en vie pour que le décodage serve. */
const decoded = new Map<string, { img: HTMLImageElement; ready: Promise<void> }>();
const KEEP = 12;

export function preload(src: string): Promise<void> {
  const hit = decoded.get(src);
  if (hit) {
    decoded.delete(src);
    decoded.set(src, hit); // le plus récent en dernier
    return hit.ready;
  }
  const img = new Image();
  img.decoding = "async";
  img.src = src;
  const ready = img.decode().catch(() => undefined);
  decoded.set(src, { img, ready });
  while (decoded.size > KEEP) {
    const oldest = decoded.keys().next().value;
    if (oldest === undefined) break;
    decoded.delete(oldest);
  }
  return ready;
}
