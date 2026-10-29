import type { TFunction } from "i18next";
import type { Mask, MaskShape } from "../store/develop";

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
  mask: Pick<Mask, "shape" | "invert">,
  sw: number,
  sh: number,
): Uint8ClampedArray<ArrayBuffer> {
  const out = new Uint8ClampedArray(width * height * 4);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const [sx, sy] = toSource((x + 0.5) / width, (y + 0.5) / height);
      const w = maskWeight(mask, sx * sw, sy * sh, sw, sh);
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
