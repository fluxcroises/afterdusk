/** Famille de réglages à cocher (preset personnel, copier-coller). */
export interface SectionOption {
  key: string;
  /** Réglages commandés (chemins du JSON : « light.exposure », « curve »…). */
  paths: string[];
}

/** Familles qu'un preset peut commander (jamais la géométrie ni les défauts). */
export const PRESET_SECTIONS: SectionOption[] = [
  { key: "exposure", paths: ["light.exposure"] },
  {
    key: "tone",
    paths: [
      "light.brightness",
      "light.contrast",
      "light.highlights",
      "light.shadows",
      "light.whites",
      "light.blacks",
    ],
  },
  { key: "whiteBalance", paths: ["color.whiteBalance"] },
  { key: "presence", paths: ["color.vibrance", "color.saturation"] },
  { key: "curve", paths: ["curve"] },
  { key: "hsl", paths: ["hsl"] },
  { key: "sharpening", paths: ["detail.sharpening"] },
  { key: "noise", paths: ["detail.noise"] },
  { key: "vignette", paths: ["effects.vignette"] },
  { key: "grain", paths: ["effects.grain"] },
  { key: "lens", paths: ["lens"] },
];

/** Cochées par défaut pour un preset : l'aspect, pas ce qui dépend de la photo. */
export const PRESET_DEFAULT_KEYS = [
  "tone",
  "presence",
  "curve",
  "hsl",
  "sharpening",
  "noise",
  "vignette",
  "grain",
];

/** Chemins des familles cochées. */
export function pathsOf(options: SectionOption[], keys: string[]): string[] {
  return options.filter((o) => keys.includes(o.key)).flatMap((o) => o.paths);
}

/** Familles du copier-coller : celles d'un preset, plus la géométrie, les défauts et les masques. */
export const COPY_SECTIONS: SectionOption[] = [
  ...PRESET_SECTIONS,
  { key: "geometry", paths: ["geometry"] },
  { key: "spots", paths: ["spots"] },
  { key: "masks", paths: ["masks"] },
];

/** Cochées par défaut : tout, sauf le recadrage, les défauts et les masques, propres à chaque photo. */
export const COPY_DEFAULT_KEYS = PRESET_SECTIONS.map((o) => o.key);
