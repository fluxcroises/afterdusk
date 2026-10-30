import type { TFunction } from "i18next";
import type { Mask, MaskShape, Stroke } from "../store/develop";

/** Adoucissement d'un dégradé radial sans valeur enregistrée. */
export const MASK_FEATHER_DEFAULT = 50;

/** Plus petite taille d'un dégradé, en fraction de l'image (comme le cœur). */
export const MASK_SIZE_MIN = 0.001;

const clamp01 = (t: number) => Math.min(1, Math.max(0, t));

/** Montée douce de 0 à 1 sur [0, 1]. */
function smoothstep(t: number): number {
  const x = clamp01(t);
  return x * x * (3 - 2 * x);
}

/**
 * Poids de 0 à 1 d'un masque au point (x, y) de la source, en pixels, pour
 * une source de `sw` × `sh` pixels. Miroir de `develop::masks` dans le cœur :
 * le rendu et l'affichage du masque s'accordent.
 */
export function maskWeight(
  mask: Pick<Mask, "shape" | "invert">,
  x: number,
  y: number,
  sw: number,
  sh: number,
): number {
  const s = mask.shape;
  let w: number;
  if (s.kind === "linear") {
    const [ax, ay] = [s.from[0] * sw, s.from[1] * sh];
    const [dx, dy] = [s.to[0] * sw - ax, s.to[1] * sh - ay];
    const len2 = Math.max(dx * dx + dy * dy, Number.EPSILON);
    w = 1 - smoothstep(((x - ax) * dx + (y - ay) * dy) / len2);
  } else if (s.kind === "brush") {
    // Pinceau : l'effet vient des tracés, dessinés par le cœur (V0.7).
    w = 0;
  } else {
    const a = ((s.angle ?? 0) * Math.PI) / 180;
    const [sin, cos] = [Math.sin(a), Math.cos(a)];
    const [dx, dy] = [x - s.center[0] * sw, y - s.center[1] * sh];
    const u = (dx * cos + dy * sin) / Math.max(s.radius[0] * sw, Number.EPSILON);
    const v = (-dx * sin + dy * cos) / Math.max(s.radius[1] * sh, Number.EPSILON);
    const q = Math.hypot(u, v);
    const inner = 1 - (s.feather ?? MASK_FEATHER_DEFAULT) / 100;
    w = q >= 1 ? 0 : q <= inner ? 1 : 1 - smoothstep((q - inner) / (1 - inner));
  }
  return mask.invert ? 1 - w : w;
}

/** Segment d'un tracé, en pixels de la source. */
interface Segment {
  ax: number;
  ay: number;
  bx: number;
  by: number;
  radius: number;
  inner: number;
  /** Boîte englobante, rayon compris. */
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

interface CompiledStroke {
  segments: Segment[];
  flow: number;
  erase: boolean;
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

/** Tracé ramené en pixels de la source, points trop proches sautés (comme le cœur). */
function compileStroke(s: Stroke, sw: number, sh: number): CompiledStroke {
  const radius = Math.max(s.radius * Math.max(sw, sh), 0.5);
  const inner = 1 - (s.feather ?? MASK_FEATHER_DEFAULT) / 100;
  const pts = s.points.map(([x, y]): [number, number] => [x * sw, y * sh]);
  const segments: Segment[] = [];
  const push = (a: [number, number], b: [number, number]) =>
    segments.push({
      ax: a[0],
      ay: a[1],
      bx: b[0],
      by: b[1],
      radius,
      inner,
      x0: Math.min(a[0], b[0]) - radius,
      y0: Math.min(a[1], b[1]) - radius,
      x1: Math.max(a[0], b[0]) + radius,
      y1: Math.max(a[1], b[1]) + radius,
    });
  const first = pts[0];
  if (first) {
    let last = first;
    for (const p of pts.slice(1)) {
      if (Math.hypot(p[0] - last[0], p[1] - last[1]) >= radius * 0.5) {
        push(last, p);
        last = p;
      }
    }
    if (segments.length === 0) push(first, first);
  }
  return {
    segments,
    flow: (s.flow ?? 100) / 100,
    erase: s.erase ?? false,
    x0: Math.min(...segments.map((g) => g.x0)),
    y0: Math.min(...segments.map((g) => g.y0)),
    x1: Math.max(...segments.map((g) => g.x1)),
    y1: Math.max(...segments.map((g) => g.y1)),
  };
}

/** Effet d'un segment au point (x, y), avant densité. */
function segmentProfile(g: Segment, x: number, y: number): number {
  if (x < g.x0 || x > g.x1 || y < g.y0 || y > g.y1) return 0;
  const [dx, dy] = [g.bx - g.ax, g.by - g.ay];
  const [px, py] = [x - g.ax, y - g.ay];
  const len2 = dx * dx + dy * dy;
  const t = len2 > 0 ? clamp01((px * dx + py * dy) / len2) : 0;
  const d2 = ((px - t * dx) ** 2 + (py - t * dy) ** 2) / (g.radius * g.radius);
  if (d2 >= 1) return 0;
  if (d2 <= g.inner * g.inner) return 1;
  return 1 - smoothstep((Math.sqrt(d2) - g.inner) / (1 - g.inner));
}

/**
 * Poids complet d'un masque, tracés compris, en un point de la source
 * (pixels) : la forme (inversée pour un dégradé), puis chaque tracé dans
 * l'ordre (ajout sans dépasser le plein effet, ou gomme), puis l'inversion
 * d'un pinceau. Miroir de `StrokeGrid::apply` dans le cœur.
 */
export function compileMask(
  mask: Pick<Mask, "shape" | "invert" | "strokes">,
  sw: number,
  sh: number,
): (x: number, y: number) => number {
  const brush = mask.shape.kind === "brush";
  const strokes = (mask.strokes ?? [])
    .filter((s) => s.points.length > 0)
    .map((s) => compileStroke(s, sw, sh));
  const shape = { shape: mask.shape, invert: brush ? false : mask.invert };
  return (x, y) => {
    let m = maskWeight(shape, x, y, sw, sh);
    for (const s of strokes) {
      if (x < s.x0 || x > s.x1 || y < s.y0 || y > s.y1) continue;
      let v = 0;
      for (const g of s.segments) {
        v = Math.max(v, segmentProfile(g, x, y));
        if (v >= 1) break;
      }
      v *= s.flow;
      m = s.erase ? m * (1 - v) : m + v * (1 - m);
    }
    return brush && mask.invert ? 1 - m : m;
  };
}

/** « Linéaire 2 » : numéroté parmi les masques de la même forme. */
export function maskName(t: TFunction, masks: Mask[], index: number): string {
  const kind = masks[index]?.shape.kind ?? "linear";
  const n = masks.slice(0, index + 1).filter((m) => m.shape.kind === kind).length;
  return t(`develop.masks.names.${kind}`, { n });
}

/** Angle ramené dans [-180, 180), au dixième de degré. */
export function normalizeAngle(angle: number): number {
  const a = ((((angle + 180) % 360) + 360) % 360) - 180;
  return Math.round(a * 10) / 10;
}

/**
 * Calque rouge du masque, `width` × `height` pixels RGBA : chaque pixel est
 * ramené au point de la source qu'il montre par `toSource` (fractions), puis
 * pondéré comme au rendu.
 */
export function maskViewPixels(
  width: number,
  height: number,
  toSource: (fx: number, fy: number) => [number, number],
  mask: Pick<Mask, "shape" | "invert" | "strokes">,
  sw: number,
  sh: number,
): Uint8ClampedArray<ArrayBuffer> {
  const out = new Uint8ClampedArray(width * height * 4);
  const weight = compileMask(mask, sw, sh);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const [sx, sy] = toSource((x + 0.5) / width, (y + 0.5) / height);
      const w = weight(sx * sw, sy * sh);
      const i = (y * width + x) * 4;
      out[i] = 230;
      out[i + 1] = 40;
      out[i + 2] = 40;
      out[i + 3] = Math.round(w * 0.5 * 255);
    }
  }
  return out;
}

/** Copie d'une forme, déplacée de (dx, dy) en fractions de la source. */
export function movedShape(shape: MaskShape, dx: number, dy: number): MaskShape {
  const add = (p: [number, number]): [number, number] => [p[0] + dx, p[1] + dy];
  if (shape.kind === "brush") return shape;
  return shape.kind === "linear"
    ? { ...shape, from: add(shape.from), to: add(shape.to) }
    : { ...shape, center: add(shape.center) };
}
