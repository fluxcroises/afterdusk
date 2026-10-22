import type { CropRect } from "../store/develop";

export type Grip = "move" | "n" | "s" | "e" | "w" | "ne" | "nw" | "se" | "sw";

/** Plus petit côté du cadre, en fraction de l'image. */
const MIN_SIDE = 0.02;

/** Nouveau cadre après un glissé de (dx, dy), en fractions de l'image. */
export function dragged(
  start: CropRect,
  grip: Grip,
  dx: number,
  dy: number,
  ratio: number | null,
  boxAspect: number,
): CropRect | null {
  const c = { ...start };
  if (grip === "move") {
    const w = c.right - c.left;
    const h = c.bottom - c.top;
    const left = Math.min(1 - w, Math.max(0, c.left + dx));
    const top = Math.min(1 - h, Math.max(0, c.top + dy));
    return { left, top, right: left + w, bottom: top + h };
  }
  if (grip.includes("w")) c.left = start.left + dx;
  if (grip.includes("e")) c.right = start.right + dx;
  if (grip.includes("n")) c.top = start.top + dy;
  if (grip.includes("s")) c.bottom = start.bottom + dy;
  if (ratio !== null) {
    // Hauteur (en fraction) d'un cadre de ce ratio pour une largeur donnée.
    const heightFor = (w: number) => (w * boxAspect) / ratio;
    const w = c.right - c.left;
    const h = c.bottom - c.top;
    if (grip === "n" || grip === "s") {
      // Côté haut ou bas : la largeur suit, centrée.
      const nw = (h * ratio) / boxAspect;
      const cx = (start.left + start.right) / 2;
      c.left = cx - nw / 2;
      c.right = cx + nw / 2;
    } else {
      const nh = heightFor(w);
      if (grip === "e" || grip === "w") {
        const cy = (start.top + start.bottom) / 2;
        c.top = cy - nh / 2;
        c.bottom = cy + nh / 2;
      } else if (grip.includes("n")) {
        c.top = c.bottom - nh;
      } else {
        c.bottom = c.top + nh;
      }
    }
  }
  const ok =
    c.left >= -1e-6 &&
    c.top >= -1e-6 &&
    c.right <= 1 + 1e-6 &&
    c.bottom <= 1 + 1e-6 &&
    c.right - c.left >= MIN_SIDE &&
    c.bottom - c.top >= MIN_SIDE;
  return ok
    ? {
        left: Math.max(0, c.left),
        top: Math.max(0, c.top),
        right: Math.min(1, c.right),
        bottom: Math.min(1, c.bottom),
      }
    : null;
}
