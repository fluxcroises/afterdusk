/** Miroir de `export::design` : document d'une composition (version 2). */

export type Layout = "grid" | "strip" | "polaroid";

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
  /** Famille de police, livrée ou installée. */
  font: string;
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
  layout: Layout;
  width: number;
  height: number;
  photos: DesignPhoto[];
  background: string;
  margin: number;
  frame: { width: number; color: string } | null;
  texts: DesignText[];
  logo: DesignLogo | null;
}

export const DESIGN_VERSION = 2;
export const MAX_PHOTOS = 4;
export const DEFAULT_FONT = "Inter";

/** Bandeau libre en bas d'une bande photomaton (fraction de la hauteur). */
export const STRIP_BAND = 0.12;
/** Photo d'un Polaroid 600 dans son cadre de 88 × 107 mm. */
export const POLAROID_CELL: Cell = [4.5 / 88, 6 / 107, 83.5 / 88, 85 / 107];

/** Disposition d'un modèle de départ. */
export function layoutOf(template: string): Layout {
  return template === "photo-strip" ? "strip" : template === "polaroid" ? "polaroid" : "grid";
}

/** Photos que tient une disposition. */
export function maxPhotos(layout: Layout): number {
  return layout === "polaroid" ? 1 : MAX_PHOTOS;
}

/** Hauteur d'une ligne de texte, en fois le corps (comme le rendu Rust). */
export const LINE_HEIGHT = 1.2;

export type Cell = [number, number, number, number];

/**
 * Cases des photos, en fractions de la page : même calcul que
 * `export::design::cells` côté Rust (le rendu final). La case `k` reçoit
 * la photo `k % n` (une bande doublée répète ses photos).
 */
export function cells(layout: Layout, n: number, aspect: number, margin: number): Cell[] {
  const a = Number.isFinite(aspect) && aspect > 0 ? aspect : 1;
  const m = Math.min(Math.max(margin, 0), 20) / 100;
  const [mx, my] = a >= 1 ? [m / a, m] : [m, m * a];
  if (layout === "polaroid") return n > 0 ? [POLAROID_CELL] : [];
  if (layout === "strip") return strip(n, a, mx, my);
  return grid(n, a, mx, my);
}

function strip(n: number, a: number, mx: number, my: number): Cell[] {
  if (n <= 0) return [];
  const columns = Math.max(1, Math.round(a * 3));
  const width = (1 - mx * (columns + 1)) / columns;
  const height = (1 - STRIP_BAND - my * n) / n;
  const out: Cell[] = [];
  for (let c = 0; c < columns; c++) {
    const left = mx + c * (width + mx);
    for (let i = 0; i < n; i++) {
      const top = my + i * (height + my);
      out.push([left, top, left + width, top + height]);
    }
  }
  return out;
}

function grid(n: number, a: number, mx: number, my: number): Cell[] {
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
