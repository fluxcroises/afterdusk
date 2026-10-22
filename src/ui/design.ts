/** Miroir de `export::design` : document d'une composition (version 1). */
export interface DesignPhoto {
  item: number;
  focus: [number, number];
}

export interface DesignText {
  text: string;
  x: number;
  y: number;
  /** Corps, en % de la hauteur de la page. */
  size: number;
  weight: number;
  color: string;
  align: "left" | "center" | "right";
}

export interface DesignLogo {
  x: number;
  y: number;
  /** Largeur, en % de la largeur de la page. */
  width: number;
  opacity: number;
}

export interface Design {
  version: number;
  template: string;
  width: number;
  height: number;
  photos: DesignPhoto[];
  background: string;
  margin: number;
  frame: { width: number; color: string } | null;
  texts: DesignText[];
  logo: DesignLogo | null;
}

export const DESIGN_VERSION = 1;
export const MAX_PHOTOS = 4;

/** Hauteur d'une ligne de texte, en fois le corps (comme le rendu Rust). */
export const LINE_HEIGHT = 1.2;

export type Cell = [number, number, number, number];

/**
 * Cases des photos, en fractions de la page : même calcul que
 * `export::design::cells` côté Rust (le rendu final).
 */
export function cells(n: number, aspect: number, margin: number): Cell[] {
  const a = Number.isFinite(aspect) && aspect > 0 ? aspect : 1;
  const m = Math.min(Math.max(margin, 0), 20) / 100;
  const [mx, my] = a >= 1 ? [m / a, m] : [m, m * a];
  const [l, t, r, b] = [mx, my, 1 - mx, 1 - my];
  const midX = (l + r) / 2;
  const midY = (t + b) / 2;
  const [gx, gy] = [mx / 2, my / 2];
  const wide = a >= 1;
  if (n <= 0) return [];
  if (n === 1) return [[l, t, r, b]];
  if (n === 2)
    return wide
      ? [
          [l, t, midX - gx, b],
          [midX + gx, t, r, b],
        ]
      : [
          [l, t, r, midY - gy],
          [l, midY + gy, r, b],
        ];
  if (n === 3)
    return wide
      ? [
          [l, t, midX - gx, b],
          [midX + gx, t, r, midY - gy],
          [midX + gx, midY + gy, r, b],
        ]
      : [
          [l, t, r, midY - gy],
          [l, midY + gy, midX - gx, b],
          [midX + gx, midY + gy, r, b],
        ];
  return [
    [l, t, midX - gx, midY - gy],
    [midX + gx, t, r, midY - gy],
    [l, midY + gy, midX - gx, b],
    [midX + gx, midY + gy, r, b],
  ];
}

/**
 * Partie de la photo (fractions) qui remplit une case de rapport `ratio`
 * autour de `focus` : même calcul que `export::templates::fit`.
 */
export function cover(
  ratio: number,
  photoAspect: number,
  focus: [number, number],
): [number, number, number, number] {
  const [fw, fh] = photoAspect > ratio ? [ratio / photoAspect, 1] : [1, photoAspect / ratio];
  const left = Math.min(Math.max(focus[0] - fw / 2, 0), 1 - fw);
  const top = Math.min(Math.max(focus[1] - fh / 2, 0), 1 - fh);
  return [left, top, left + fw, top + fh];
}
