import type { Geometry } from "../store/develop";

/**
 * Passage entre l'image affichée (après géométrie, fractions de 0 à 1) et la
 * *source* (image avant géométrie, orientation du boîtier appliquée), repère
 * des corrections de défauts. Miroir de `develop::geometry::canvas_to_source`
 * dans le cœur : quarts de tour et miroirs, redressement autour du centre,
 * recadrage.
 */

const oriented = (sw: number, sh: number, quarter: number): [number, number] =>
  quarter % 2 === 1 ? [sh, sw] : [sw, sh];

/** Point du canevas (pixels) vers le point de la source (pixels). */
function canvasToSource(p: [number, number], g: Geometry, sw: number, sh: number) {
  const quarter = g.quarter ?? 0;
  const [w, h] = oriented(sw, sh, quarter);
  const [cx, cy] = [w / 2, h / 2];
  const a = ((g.angle ?? 0) * Math.PI) / 180;
  const [s, c] = [Math.sin(a), Math.cos(a)];
  const [dx, dy] = [p[0] - cx, p[1] - cy];
  let x = cx + dx * c + dy * s;
  let y = cy - dx * s + dy * c;
  if (g.flipH) x = w - x;
  if (g.flipV) y = h - y;
  switch (quarter % 4) {
    case 1:
      return [y, sh - x];
    case 2:
      return [sw - x, sh - y];
    case 3:
      return [sw - y, x];
    default:
      return [x, y];
  }
}

/** Point de la source (pixels) vers le point du canevas (pixels). */
function sourceToCanvas(p: [number, number], g: Geometry, sw: number, sh: number) {
  const quarter = g.quarter ?? 0;
  const [w, h] = oriented(sw, sh, quarter);
  const [u, v] = p;
  let [x, y] =
    quarter % 4 === 1
      ? [sh - v, u]
      : quarter % 4 === 2
        ? [sw - u, sh - v]
        : quarter % 4 === 3
          ? [v, sw - u]
          : [u, v];
  if (g.flipH) x = w - x;
  if (g.flipV) y = h - y;
  const [cx, cy] = [w / 2, h / 2];
  const a = ((g.angle ?? 0) * Math.PI) / 180;
  const [s, c] = [Math.sin(a), Math.cos(a)];
  return [cx + c * (x - cx) - s * (y - cy), cy + s * (x - cx) + c * (y - cy)];
}

/** Taille de la sortie (recadrage du canevas), en pixels de la source. */
export function outputSize(g: Geometry, sw: number, sh: number): [number, number] {
  const [w, h] = oriented(sw, sh, g.quarter ?? 0);
  const c = g.crop ?? { left: 0, top: 0, right: 1, bottom: 1 };
  return [(c.right - c.left) * w, (c.bottom - c.top) * h];
}

/** Point de l'image affichée (fractions) vers la source (fractions). */
export function outputToSource(
  fx: number,
  fy: number,
  g: Geometry,
  sw: number,
  sh: number,
): [number, number] {
  const [w, h] = oriented(sw, sh, g.quarter ?? 0);
  const c = g.crop ?? { left: 0, top: 0, right: 1, bottom: 1 };
  const canvas: [number, number] = [
    (c.left + fx * (c.right - c.left)) * w,
    (c.top + fy * (c.bottom - c.top)) * h,
  ];
  const [x, y] = canvasToSource(canvas, g, sw, sh);
  return [x / sw, y / sh];
}

/** Point de la source (fractions) vers l'image affichée (fractions). */
export function sourceToOutput(
  sx: number,
  sy: number,
  g: Geometry,
  sw: number,
  sh: number,
): [number, number] {
  const [w, h] = oriented(sw, sh, g.quarter ?? 0);
  const c = g.crop ?? { left: 0, top: 0, right: 1, bottom: 1 };
  const [x, y] = sourceToCanvas([sx * sw, sy * sh], g, sw, sh);
  return [(x / w - c.left) / (c.right - c.left), (y / h - c.top) / (c.bottom - c.top)];
}
