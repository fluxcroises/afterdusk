import i18next from "i18next";
import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";
import { type CommandError, toCommandError } from "../api/core";

export type WhiteBalance =
  | { mode: "asShot" }
  | { mode: "preset"; preset: string }
  | { mode: "custom"; temperature: number; tint: number };

export const LIGHT_KEYS = [
  "exposure",
  "brightness",
  "contrast",
  "highlights",
  "shadows",
  "whites",
  "blacks",
] as const;
export type LightKey = (typeof LIGHT_KEYS)[number];

export const COLOR_KEYS = ["vibrance", "saturation"] as const;
export type ColorKey = (typeof COLOR_KEYS)[number];

export const CURVE_CHANNELS = ["rgb", "red", "green", "blue"] as const;
export type CurveChannel = (typeof CURVE_CHANNELS)[number];
/** Point d'une courbe : entrée et sortie de 0 à 255. */
export type CurvePoint = [number, number];

export const HUES = [
  "red",
  "orange",
  "yellow",
  "green",
  "aqua",
  "blue",
  "purple",
  "magenta",
] as const;
export type Hue = (typeof HUES)[number];
export const HSL_KINDS = ["hue", "saturation", "luminance"] as const;
export type HslKind = (typeof HSL_KINDS)[number];

/** Effets et leurs valeurs par défaut (absentes du JSON creux). */
export const EFFECT_DEFAULTS = {
  vignette: { amount: 0, midpoint: 50, feather: 50 },
  grain: { amount: 0, size: 25, roughness: 50 },
} as const;
export type EffectGroup = keyof typeof EFFECT_DEFAULTS;

/** Netteté et réduction du bruit, valeurs par défaut (absentes du JSON creux). */
export const DETAIL_DEFAULTS = {
  sharpening: { amount: 0, radius: 1, detail: 25, masking: 0 },
  noise: { luminance: 0, luminanceDetail: 50, color: 0 },
} as const;
export type DetailGroup = keyof typeof DETAIL_DEFAULTS;
export type EffectKey<G extends EffectGroup> = keyof (typeof EFFECT_DEFAULTS)[G];

/** Rectangle de recadrage, en fractions de l'image orientée et redressée. */
export interface CropRect {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

export const FULL_CROP: CropRect = { left: 0, top: 0, right: 1, bottom: 1 };

/** Géométrie, miroir de `develop::settings::Geometry` (JSON creux). */
export interface Geometry {
  crop?: CropRect;
  angle?: number;
  quarter?: number;
  flipH?: boolean;
  flipV?: boolean;
  aspect?: string;
}

/** Ratios proposés par l'outil de recadrage (largeur:hauteur, en paysage). */
export const ASPECTS = ["free", "original", "1:1", "4:5", "3:2", "16:9"] as const;
export type Aspect = (typeof ASPECTS)[number];

/** Rapport largeur / hauteur d'un ratio, `original` étant celui du canevas. */
export function aspectRatio(aspect: string | undefined, canvas: number): number | null {
  if (!aspect || aspect === "free") return null;
  if (aspect === "original") return canvas;
  const [a, b] = aspect.split(":").map(Number);
  return a && b ? a / b : null;
}

/** Cadre arrondi à 1/10 000, borné à l'image : pas de dérive après des allers-retours. */
function roundCrop(c: CropRect): CropRect {
  const r = (v: number) => Math.round(Math.min(1, Math.max(0, v)) * 10000) / 10000;
  return { left: r(c.left), top: r(c.top), right: r(c.right), bottom: r(c.bottom) };
}

function isFullCrop(c: CropRect | undefined): boolean {
  return !c || (c.left === 0 && c.top === 0 && c.right === 1 && c.bottom === 1);
}

/** Géométrie creuse : les valeurs par défaut disparaissent. */
function sparseGeometry(g: Geometry): Geometry | undefined {
  const out: Geometry = {};
  const crop = g.crop && roundCrop(g.crop);
  if (!isFullCrop(crop) && crop) out.crop = crop;
  if (g.angle) out.angle = g.angle;
  if (g.quarter) out.quarter = g.quarter;
  if (g.flipH) out.flipH = true;
  if (g.flipV) out.flipV = true;
  if (g.aspect && g.aspect !== "free") out.aspect = g.aspect;
  return Object.keys(out).length > 0 ? out : undefined;
}

/** Corrections d'objectif, miroir de `develop::settings::LensSettings`. */
export interface LensSettings {
  enabled: boolean;
  distortion: number;
  vignetting: number;
}

export const LENS_DEFAULTS: LensSettings = { enabled: true, distortion: 100, vignetting: 100 };

export type SpotMode = "heal" | "clone";

/**
 * Correction de défaut, miroir de `develop::settings::Spot` : positions en
 * fractions de la source (image avant géométrie), rayon en fraction de son
 * côté long.
 */
export interface Spot {
  x: number;
  y: number;
  radius: number;
  sourceX: number;
  sourceY: number;
  /** Adoucissement de 0 à 100 ; absent : 50. */
  feather?: number;
  /** Absent : correction. */
  mode?: SpotMode;
}

export const SPOT_RADIUS = { min: 0.001, max: 0.1 } as const;
export const SPOT_FEATHER_DEFAULT = 50;

/** Pinceau du correcteur : réglages de la prochaine correction. */
export interface SpotBrush {
  radius: number;
  feather: number;
  mode: SpotMode;
}

/** Outil de la barre de droite. */
export type DevelopTool = "adjust" | "filters" | "crop" | "spots";

/** Pellicule évoquée par une simulation de film (boîte dessinée, sans marque). */
export interface Film {
  kind: "negative" | "slide" | "bw";
  iso: number;
  colors: [string, string];
}

/** Preset de la galerie, miroir de `develop::presets::PresetSummary`. */
export interface PresetSummary {
  id: string;
  /** Nom d'un preset personnel ; les presets fournis sont nommés par l'interface. */
  name: string | null;
  film: Film | null;
  controls: string[];
}

export interface PresetGroup {
  id: string;
  user: boolean;
  presets: PresetSummary[];
}

/** Vignette d'un preset rendue sur la photo ouverte. */
export interface PresetThumb {
  width: number;
  height: number;
  pixels: Uint8ClampedArray<ArrayBuffer>;
}

/** Vignettes renvoyées par `develop_preset_previews`. */
export function parseThumbs(buffer: ArrayBuffer): PresetThumb[] {
  const view = new DataView(buffer);
  const count = view.getUint32(0, true);
  const out: PresetThumb[] = [];
  let at = 4;
  for (let i = 0; i < count; i++) {
    const width = view.getUint32(at, true);
    const height = view.getUint32(at + 4, true);
    at += 8;
    const size = width * height * 4;
    out.push({ width, height, pixels: new Uint8ClampedArray(buffer.slice(at, at + size)) });
    at += size;
  }
  return out;
}

/** Délai après un réglage avant de refaire les vignettes de la galerie. */
export const THUMBS_DELAY_MS = 400;

const r5 = (v: number) => Math.round(v * 100000) / 100000;

/** Correction creuse : les valeurs par défaut disparaissent. */
function sparseSpot(spot: Spot): Spot {
  const out: Spot = {
    x: r5(spot.x),
    y: r5(spot.y),
    radius: r5(spot.radius),
    sourceX: r5(spot.sourceX),
    sourceY: r5(spot.sourceY),
  };
  if (spot.feather !== undefined && spot.feather !== SPOT_FEATHER_DEFAULT) {
    out.feather = spot.feather;
  }
  if (spot.mode === "clone") out.mode = "clone";
  return out;
}

/** Réglages de retouche, miroir de `develop::settings::DevelopSettings` (JSON creux). */
export interface DevelopSettings {
  version: number;
  process: number;
  light?: Partial<Record<LightKey, number>>;
  color?: Partial<Record<ColorKey, number>> & { whiteBalance?: WhiteBalance };
  curve?: Partial<Record<CurveChannel, CurvePoint[]>>;
  hsl?: Partial<Record<HslKind, Partial<Record<Hue, number>>>>;
  effects?: {
    vignette?: Partial<Record<EffectKey<"vignette">, number>>;
    grain?: Partial<Record<EffectKey<"grain">, number>>;
  };
  geometry?: Geometry;
  /** Dernier preset appliqué par la galerie, et son intensité (%). */
  look?: { id: string; amount: number };
  /** Corrections d'objectif ; absentes : actives, à 100 % du profil. */
  lens?: Partial<LensSettings>;
  spots?: Spot[];
  detail?: {
    sharpening?: Partial<Record<keyof (typeof DETAIL_DEFAULTS)["sharpening"], number>>;
    noise?: Partial<Record<keyof (typeof DETAIL_DEFAULTS)["noise"], number>>;
  };
}

/** Courbe identité : deux points aux extrémités. */
export const IDENTITY_CURVE: CurvePoint[] = [
  [0, 0],
  [255, 255],
];

function isIdentityCurve(points: CurvePoint[]): boolean {
  return (
    points.length < 2 ||
    (points.length === 2 &&
      points[0]?.[0] === 0 &&
      points[0]?.[1] === 0 &&
      points[1]?.[0] === 255 &&
      points[1]?.[1] === 255)
  );
}

/** Copie de `obj` avec `key` à `value`, ou sans `key` si `value` est absente. */
function put<T extends object, K extends keyof T>(obj: T, key: K, value: T[K] | undefined): T {
  const rest = Object.fromEntries(Object.entries(obj).filter(([k]) => k !== key)) as T;
  return value === undefined ? rest : { ...rest, [key]: value };
}

/** Retire les sections devenues vides, pour un JSON creux. */
function prune<T extends object>(section: T): T | undefined {
  return Object.keys(section).length === 0 ? undefined : section;
}

export interface WhitePoint {
  temperature: number;
  tint: number;
}

export interface WhitePreset extends WhitePoint {
  name: string;
}

interface DevelopOpened {
  mediaId: number;
  width: number;
  height: number;
  sourceWidth: number;
  sourceHeight: number;
  lens: string | null;
  settings: DevelopSettings;
  asShot: WhitePoint;
  presets: WhitePreset[];
  adapter: string | null;
  decodeMs: number;
}

/** Rendu reçu du cœur : en-tête de 16 octets, histogramme, pixels RGBA. */
export interface DevelopFrame {
  width: number;
  height: number;
  /** Durée du rendu dans le cœur, en millisecondes. */
  renderMs: number;
  gpu: boolean;
  /** 4 × 256 cases : rouge, vert, bleu, luminance. */
  histogram: Uint32Array<ArrayBuffer>;
  pixels: Uint8ClampedArray<ArrayBuffer>;
}

const HEADER_BYTES = 16;
export const HISTOGRAM_BINS = 1024;

export function parseFrame(buffer: ArrayBuffer): DevelopFrame {
  const header = new DataView(buffer, 0, HEADER_BYTES);
  const width = header.getUint32(0, true);
  const height = header.getUint32(4, true);
  return {
    width,
    height,
    renderMs: header.getUint32(8, true) / 1000,
    gpu: header.getUint32(12, true) === 1,
    histogram: new Uint32Array(buffer, HEADER_BYTES, HISTOGRAM_BINS),
    pixels: new Uint8ClampedArray(buffer, HEADER_BYTES + HISTOGRAM_BINS * 4, width * height * 4),
  };
}

/** Plages des curseurs (décision 0005). */
export const EXPOSURE_RANGE = { min: -5, max: 5, step: 0.05 } as const;
export const SLIDER_RANGE = { min: -100, max: 100, step: 1 } as const;
export const TEMPERATURE_RANGE = { min: 2000, max: 50000 } as const;
export const TINT_RANGE = { min: -150, max: 150, step: 1 } as const;

/**
 * Curseur de température : régulier en mireds (1e6 / K), comme l'œil perçoit
 * les écarts de couleur ; `position` va de 0 (chaud) à 1 (froid).
 */
export function temperatureFromPosition(position: number): number {
  const hot = 1e6 / TEMPERATURE_RANGE.min;
  const cold = 1e6 / TEMPERATURE_RANGE.max;
  const mired = hot - Math.min(1, Math.max(0, position)) * (hot - cold);
  // Arrondi à 50 K : assez fin pour l'œil, lisible.
  return Math.round(1e6 / mired / 50) * 50;
}

export function positionFromTemperature(kelvin: number): number {
  const hot = 1e6 / TEMPERATURE_RANGE.min;
  const cold = 1e6 / TEMPERATURE_RANGE.max;
  return (hot - 1e6 / kelvin) / (hot - cold);
}

/** Délai après le dernier geste avant l'enregistrement (historique et XMP). */
export const SAVE_DELAY_MS = 600;

/** Une étape de l'historique, miroir de `develop::store::HistoryStep`. */
export interface HistoryStep {
  position: number;
  label: string;
  settings: DevelopSettings;
  createdAt: string;
}

export interface History {
  steps: HistoryStep[];
  /** Étape affichée ; 0 : l'original. */
  current: number;
}

/** Un instantané nommé d'une version, miroir de `develop::store::Snapshot`. */
export interface Snapshot {
  name: string;
  settings: DevelopSettings;
  createdAt: string;
}

/** Une copie virtuelle, miroir de `develop::store::Version`. */
export interface Version {
  name: string;
  createdAt: string;
}

/** Nom de la version originale. */
export const ORIGINAL = "";

interface VersionState {
  settings: DevelopSettings;
  history: History;
  snapshots: Snapshot[];
  versions: Version[];
}

/** Premier nom libre de la forme « base n ». */
export function freeName(base: (n: number) => string, taken: string[]): string {
  for (let n = 1; ; n++) {
    const name = base(n);
    if (!taken.includes(name)) return name;
  }
}

/** Avant/après : rendu retouché, original seul, ou les deux côte à côte. */
export type CompareMode = "off" | "before" | "split";

/** Zoom 100 % : coin haut gauche de la région affichée, en pixels de l'original. */
export interface ZoomView {
  left: number;
  top: number;
}

export interface ClippingWarnings {
  highlights: boolean;
  shadows: boolean;
}

interface DevelopState {
  mediaId: number | null;
  loading: boolean;
  error: CommandError | null;
  settings: DevelopSettings | null;
  asShot: WhitePoint | null;
  presets: WhitePreset[];
  adapter: string | null;
  decodeMs: number | null;
  frame: DevelopFrame | null;
  /** Temps entre le geste et l'image reçue, interface comprise. */
  roundTripMs: number | null;
  clipping: ClippingWarnings;
  /** Pipette active : le prochain clic sur la photo règle la balance. */
  picking: boolean;
  /** Calcul automatique en cours. */
  autoBusy: boolean;
  history: History | null;
  compare: CompareMode;
  /** Rendu de l'original (réglages neutres), pour l'avant/après. */
  beforeFrame: DevelopFrame | null;
  /** Réglages copiés (copier-coller) et réglages à transmettre. */
  clipboard: { settings: DevelopSettings; controls: string[] } | null;
  /** Boîte « Copier les réglages » ouverte. */
  copyOpen: boolean;
  /** Message bref de la barre d'outils (« Réglages collés sur 3 photos »). */
  notice: string | null;
  /** Presets de la galerie, chargés à la première ouverture. */
  presetGroups: PresetGroup[] | null;
  /** Vignettes par identifiant de preset (« » : l'original). */
  presetThumbs: Record<string, PresetThumb>;
  /** Objectif reconnu dans la base lensfun, ou rien. */
  lensName: string | null;
  /** Outil choisi dans la barre de droite. */
  tool: DevelopTool;
  /** Image avant géométrie (aperçu), repère des corrections de défauts. */
  sourceSize: { width: number; height: number } | null;
  spotBrush: SpotBrush;
  /** Correction choisie (indice dans `settings.spots`). */
  selectedSpot: number | null;
  /** Visualisation des poussières, dans l'outil de correction. */
  showDust: boolean;
  /** Outil de recadrage ouvert : l'image entière redressée est affichée. */
  cropping: boolean;
  /** Position de l'historique à l'ouverture de l'outil, pour l'annuler. */
  cropStart: number | null;
  /** Zoom 100 % en cours, ou rien (photo ajustée à l'écran). */
  zoom: ZoomView | null;
  /** Dimensions de l'original en pleine définition, une fois décodé. */
  fullSize: { width: number; height: number } | null;
  zoomLoading: boolean;
  zoomFrame: DevelopFrame | null;
  /** Taille de la zone d'affichage, en pixels de l'écran (densité comprise). */
  viewport: { width: number; height: number };
  /** Version affichée : l'originale ("") ou une copie virtuelle. */
  version: string;
  versions: Version[];
  snapshots: Snapshot[];
  open: (mediaId: number) => Promise<void>;
  setLight: (key: LightKey, value: number) => void;
  setColor: (key: ColorKey, value: number) => void;
  setWhiteBalance: (wb: WhiteBalance) => void;
  setTemperature: (kelvin: number) => void;
  setTint: (tint: number) => void;
  setPicking: (picking: boolean) => void;
  pickWhite: (x: number, y: number) => Promise<void>;
  toggleClipping: (which: keyof ClippingWarnings) => void;
  /** Courbe d'un canal ; l'identité retire la courbe. */
  setCurve: (channel: CurveChannel, points: CurvePoint[]) => void;
  setHsl: (kind: HslKind, hue: Hue, value: number) => void;
  setEffect: <G extends EffectGroup>(group: G, key: EffectKey<G>, value: number) => void;
  /** Netteté ou réduction du bruit ; le rayon garde une décimale. */
  setDetail: (group: DetailGroup, key: string, value: number) => void;
  /** Corrections d'objectif : activation, distorsion et vignetage (%). */
  setLens: <K extends keyof LensSettings>(key: K, value: LensSettings[K]) => void;
  /** Auto de la section Lumière, ou d'un seul curseur. */
  autoLight: (key?: LightKey) => Promise<void>;
  autoWhite: () => Promise<void>;
  /** Revient à une étape de l'historique (0 : l'original). */
  goTo: (position: number) => Promise<void>;
  undo: () => Promise<void>;
  redo: () => Promise<void>;
  setCompare: (mode: CompareMode) => void;
  setTool: (tool: DevelopTool) => Promise<void>;
  loadPresets: () => Promise<void>;
  setCopyOpen: (open: boolean) => void;
  /** Copie les réglages affichés, pour les familles `controls`. */
  copySettings: (controls: string[]) => void;
  /** Colle les réglages copiés sur ces photos (la photo ouverte comprise ou non). */
  pasteSettings: (mediaIds: number[]) => Promise<void>;
  /** Applique un preset (`null` : l'original) ; l'intensité par défaut est 100 %. */
  applyPreset: (id: string | null, amount?: number) => Promise<void>;
  /** Intensité du preset appliqué, de 0 à 100 %. */
  setPresetAmount: (amount: number) => Promise<void>;
  /** Nouveau preset personnel depuis les réglages affichés. */
  createPreset: (name: string, controls: string[]) => Promise<boolean>;
  renamePreset: (id: string, name: string) => Promise<boolean>;
  deletePreset: (id: string) => Promise<void>;
  /** Nouvelle correction en (x, y), fractions de la source ; sa source est
   * choisie par le cœur. */
  addSpot: (x: number, y: number) => Promise<void>;
  /** Déplace une correction (`target`) ou sa source. */
  moveSpot: (index: number, part: "target" | "source", x: number, y: number) => Promise<void>;
  /** Règle le pinceau, et la correction choisie s'il y en a une. */
  setSpotBrush: (patch: Partial<SpotBrush>) => void;
  selectSpot: (index: number | null) => void;
  removeSpot: (index: number) => Promise<void>;
  clearSpots: () => Promise<void>;
  setShowDust: (show: boolean) => void;
  startCrop: () => Promise<void>;
  finishCrop: () => void;
  cancelCrop: () => Promise<void>;
  setCrop: (crop: CropRect) => Promise<void>;
  setAngle: (angle: number, label?: string) => Promise<void>;
  autoStraighten: () => Promise<void>;
  /** Quart de tour : +1 dans le sens horaire, -1 dans l'autre. */
  rotateQuarter: (direction: 1 | -1) => void;
  flip: (axis: "h" | "v") => void;
  setAspect: (aspect: string, portrait?: boolean) => Promise<void>;
  resetGeometry: () => void;
  /** Zoom 100 % centré sur un point de la photo (fractions de 0 à 1). */
  zoomAt: (fx: number, fy: number) => Promise<void>;
  /** Déplace la région zoomée, en pixels de l'écran. */
  panZoom: (dx: number, dy: number) => void;
  exitZoom: () => void;
  setViewport: (width: number, height: number) => void;
  selectVersion: (name: string) => Promise<void>;
  /** Copie virtuelle des réglages affichés, aussitôt affichée. */
  createVersion: (name: string) => Promise<void>;
  renameVersion: (from: string, to: string) => Promise<boolean>;
  deleteVersion: (name: string) => Promise<void>;
  createSnapshot: (name: string) => Promise<void>;
  applySnapshot: (name: string) => Promise<void>;
  renameSnapshot: (from: string, to: string) => Promise<boolean>;
  deleteSnapshot: (name: string) => Promise<void>;
  /** Enregistre tout de suite un réglage en attente (changement de photo, sortie). */
  flush: () => Promise<void>;
  close: () => Promise<void>;
}

/** Température et teinte effectives d'un réglage de balance. */
export function whitePoint(
  wb: WhiteBalance | undefined,
  asShot: WhitePoint,
  presets: WhitePreset[],
): WhitePoint {
  if (!wb || wb.mode === "asShot") return asShot;
  if (wb.mode === "custom") return { temperature: wb.temperature, tint: wb.tint };
  return presets.find((p) => p.name === wb.preset) ?? asShot;
}

const round = (value: number, step: number) => Math.round(value / step) * step;

/** Section du JSON creux avec une valeur de plus : zéro, c'est l'absence de la clé. */
function withValue<T extends object>(section: T | undefined, key: string, value: number): T {
  const entries = Object.entries({ ...section, [key]: value });
  return Object.fromEntries(entries.filter(([k, v]) => k !== key || v !== 0)) as T;
}

let openToken = 0;
let noticeTimer: ReturnType<typeof setTimeout> | null = null;
/** Durée d'affichage d'un message de la barre d'outils. */
const NOTICE_MS = 4000;
let rendering = false;
let renderAgain = false;
let zoomRendering = false;
let zoomAgain = false;
/** Décodage pleine définition en cours, pour une photo donnée : partagé
 * entre la préparation en arrière-plan et le premier zoom. */
let fullDecode: { mediaId: number; promise: Promise<boolean> } | null = null;
let prepareTimer: ReturnType<typeof setTimeout> | null = null;

/** Délai sur une même photo avant de préparer le zoom en arrière-plan. */
export const ZOOM_PREPARE_DELAY_MS = 1000;
let saveTimer: ReturnType<typeof setTimeout> | null = null;
let pendingLabel: string | null = null;

export const useDevelop = create<DevelopState>()((set, get) => {
  /** Un seul rendu à la fois ; les gestes reçus entre-temps n'en déclenchent qu'un. */
  /** Réglages rendus : dans l'outil de recadrage, l'image entière redressée. */
  function renderedSettings(): DevelopSettings | null {
    const { settings, cropping } = get();
    if (!settings || !cropping || !settings.geometry?.crop) return settings;
    return put(settings, "geometry", sparseGeometry({ ...settings.geometry, crop: FULL_CROP }));
  }

  let thumbsTimer: ReturnType<typeof setTimeout> | null = null;
  let thumbsRunning = false;
  let thumbsAgain = false;

  /** Toutes les vignettes de la galerie, l'original compris. */
  async function refreshThumbs() {
    if (thumbsRunning) {
      thumbsAgain = true;
      return;
    }
    thumbsRunning = true;
    try {
      do {
        thumbsAgain = false;
        const { settings, mediaId, presetGroups, tool } = get();
        if (!settings || mediaId === null || !presetGroups || tool !== "filters") return;
        const ids = ["", ...presetGroups.flatMap((g) => g.presets.map((p) => p.id))];
        const size = Math.round(160 * (window.devicePixelRatio || 1));
        const buffer = await invoke<ArrayBuffer>("develop_preset_previews", {
          settings: renderedSettings(),
          ids,
          amount: 100,
          size,
        });
        if (get().mediaId !== mediaId) return;
        const thumbs = parseThumbs(buffer);
        set({ presetThumbs: Object.fromEntries(ids.map((id, i) => [id, thumbs[i]])) });
      } while (thumbsAgain);
    } catch (e) {
      set({ error: toCommandError(e) });
    } finally {
      thumbsRunning = false;
    }
  }

  function scheduleThumbs() {
    if (get().tool !== "filters") return;
    if (thumbsTimer) clearTimeout(thumbsTimer);
    thumbsTimer = setTimeout(() => void refreshThumbs(), THUMBS_DELAY_MS);
  }

  /** Libellé d'historique d'un preset : son identifiant, et son nom s'il est personnel. */
  function lookLabel(id: string | null): string {
    if (id === null) return "look.remove";
    const name = get()
      .presetGroups?.flatMap((g) => g.presets)
      .find((p) => p.id === id)?.name;
    return name ? `look:${id}:${name}` : `look:${id}`;
  }

  /** Avertissements demandés, et visualisation des poussières dans l'outil. */
  function displayClipping() {
    const { clipping, tool, showDust } = get();
    return { ...clipping, dust: tool === "spots" && showDust };
  }

  /** Nouvelle liste de corrections, en une étape d'historique à part. */
  async function changeSpots(spots: Spot[], label: string) {
    await get().flush();
    const { settings } = get();
    if (!settings) return;
    change(put(settings, "spots", spots.length > 0 ? spots : undefined), label);
  }

  /** Nouvelle géométrie : le zoom et ses dimensions sont à refaire. */
  function changeGeometry(geometry: Geometry, label: string) {
    const { settings } = get();
    if (!settings) return;
    set({ zoom: null, zoomFrame: null, fullSize: null });
    change(put(settings, "geometry", sparseGeometry(geometry)), label);
  }

  /** Recadrage ramené dans l'image redressée, par le cœur. */
  async function constrained(geometry: Geometry): Promise<Geometry> {
    if (!geometry.angle) return geometry;
    try {
      const g = await invoke<Geometry>("develop_geometry_constrain", {
        geometry: { ...geometry, crop: geometry.crop ?? FULL_CROP },
      });
      return { ...geometry, crop: g.crop };
    } catch (e) {
      set({ error: toCommandError(e) });
      return geometry;
    }
  }

  /** Décode l'original en pleine définition (une fois par photo). */
  function ensureFull(mediaId: number): Promise<boolean> {
    if (get().fullSize) return Promise.resolve(true);
    if (fullDecode?.mediaId === mediaId) return fullDecode.promise;
    const promise = invoke<{ width: number; height: number }>("develop_zoom_open", {
      geometry: get().settings?.geometry ?? null,
    })
      .then((opened) => {
        if (get().mediaId !== mediaId) return false;
        set({ fullSize: { width: opened.width, height: opened.height } });
        return true;
      })
      .catch((e: unknown) => {
        if (get().mediaId === mediaId) set({ error: toCommandError(e) });
        return false;
      })
      .finally(() => {
        if (fullDecode?.mediaId === mediaId) fullDecode = null;
      });
    fullDecode = { mediaId, promise };
    return promise;
  }

  /** Région zoomée bornée à l'image. */
  function clampZoom(view: ZoomView): ZoomView {
    const { fullSize, viewport } = get();
    if (!fullSize) return view;
    const maxLeft = Math.max(0, fullSize.width - viewport.width);
    const maxTop = Math.max(0, fullSize.height - viewport.height);
    return {
      left: Math.round(Math.min(maxLeft, Math.max(0, view.left))),
      top: Math.round(Math.min(maxTop, Math.max(0, view.top))),
    };
  }

  /** Rendu de la région zoomée ; un seul à la fois, comme l'aperçu. */
  async function renderZoom() {
    if (zoomRendering) {
      zoomAgain = true;
      return;
    }
    zoomRendering = true;
    try {
      do {
        zoomAgain = false;
        const { settings, mediaId, zoom, viewport } = get();
        if (!settings || mediaId === null || !zoom) return;
        const buffer = await invoke<ArrayBuffer>("develop_zoom_render", {
          settings,
          clipping: displayClipping(),
          left: zoom.left,
          top: zoom.top,
          width: Math.max(1, Math.round(viewport.width)),
          height: Math.max(1, Math.round(viewport.height)),
        });
        if (get().mediaId !== mediaId || !get().zoom) return;
        set({ zoomFrame: parseFrame(buffer) });
      } while (zoomAgain);
    } catch (e) {
      set({ error: toCommandError(e) });
    } finally {
      zoomRendering = false;
    }
  }

  async function render() {
    // L'aperçu donne toujours l'histogramme ; la région zoomée suit.
    if (get().zoom) void renderZoom();
    if (rendering) {
      renderAgain = true;
      return;
    }
    rendering = true;
    try {
      do {
        renderAgain = false;
        const { settings, mediaId } = get();
        if (!settings || mediaId === null) return;
        const started = performance.now();
        const buffer = await invoke<ArrayBuffer>("develop_render", {
          settings: renderedSettings(),
          clipping: displayClipping(),
        });
        if (get().mediaId !== mediaId) return;
        set({ frame: parseFrame(buffer), roundTripMs: performance.now() - started });
      } while (renderAgain);
    } catch (e) {
      set({ error: toCommandError(e) });
    } finally {
      rendering = false;
    }
  }

  /** Remplace les réglages, relance le rendu et prépare l'enregistrement. */
  function change(settings: DevelopSettings, label: string) {
    // Un autre réglage que le précédent : le précédent est enregistré d'abord,
    // tel quel, pour que chaque réglage ait sa propre étape d'historique.
    if (pendingLabel && pendingLabel !== label) void get().flush();
    set({ settings });
    void render();
    pendingLabel = label;
    scheduleThumbs();
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => void get().flush(), SAVE_DELAY_MS);
  }

  async function loadHistory(mediaId: number) {
    const { version } = get();
    try {
      const history = await invoke<History>("develop_history", { mediaId, version });
      if (get().mediaId === mediaId && get().version === version) set({ history });
    } catch (e) {
      set({ error: toCommandError(e) });
    }
  }

  /** Réglages, historique, instantanés et copies de la version affichée. */
  async function loadState(mediaId: number) {
    const { version } = get();
    try {
      const state = await invoke<VersionState>("develop_state", { mediaId, version });
      if (get().mediaId !== mediaId || get().version !== version) return;
      set({
        settings: state.settings,
        history: state.history,
        snapshots: state.snapshots,
        versions: state.versions,
      });
    } catch (e) {
      set({ error: toCommandError(e) });
    }
  }

  /** Opération sur les versions ou les instantanés ; `false` si refusée. */
  async function run(command: string, args: Record<string, unknown>): Promise<boolean> {
    const { mediaId } = get();
    if (mediaId === null) return false;
    try {
      await invoke(command, { mediaId, ...args });
      return true;
    } catch (e) {
      set({ error: toCommandError(e) });
      return false;
    }
  }

  /** Rendu de l'original, une fois par photo, à la première demande. */
  async function loadBefore() {
    const { mediaId, settings, beforeFrame } = get();
    if (mediaId === null || !settings || beforeFrame) return;
    const neutral = { version: settings.version, process: settings.process };
    try {
      const buffer = await invoke<ArrayBuffer>("develop_render", { settings: neutral });
      if (get().mediaId === mediaId) set({ beforeFrame: parseFrame(buffer) });
    } catch (e) {
      set({ error: toCommandError(e) });
    }
  }

  function applyWhiteBalance(wb: WhiteBalance, label: string) {
    const { settings } = get();
    if (!settings) return;
    const color = { ...settings.color };
    if (wb.mode === "asShot") delete color.whiteBalance;
    else color.whiteBalance = wb;
    change({ ...settings, color }, label);
  }

  /** Calcul automatique dans le cœur, ignoré si la photo a changé entre-temps. */
  async function computeAuto<T>(command: string, args: Record<string, unknown>) {
    const { mediaId } = get();
    set({ autoBusy: true });
    try {
      const result = await invoke<T>(command, args);
      return get().mediaId === mediaId ? result : null;
    } catch (e) {
      set({ error: toCommandError(e) });
      return null;
    } finally {
      set({ autoBusy: false });
    }
  }

  return {
    mediaId: null,
    loading: false,
    error: null,
    settings: null,
    asShot: null,
    presets: [],
    adapter: null,
    decodeMs: null,
    frame: null,
    roundTripMs: null,
    clipping: { highlights: false, shadows: false },
    picking: false,
    autoBusy: false,
    history: null,
    compare: "off",
    beforeFrame: null,
    version: ORIGINAL,
    versions: [],
    snapshots: [],
    zoom: null,
    fullSize: null,
    zoomLoading: false,
    zoomFrame: null,
    viewport: { width: 1, height: 1 },
    cropping: false,
    cropStart: null,
    tool: "adjust",
    clipboard: null,
    copyOpen: false,
    notice: null,
    presetGroups: null,
    presetThumbs: {},
    lensName: null,
    sourceSize: null,
    spotBrush: { radius: 0.01, feather: SPOT_FEATHER_DEFAULT, mode: "heal" },
    selectedSpot: null,
    showDust: false,

    open: async (mediaId) => {
      if (get().mediaId === mediaId && get().settings) return;
      await get().flush();
      const token = ++openToken;
      set({
        mediaId,
        loading: true,
        error: null,
        settings: null,
        frame: null,
        picking: false,
        history: null,
        beforeFrame: null,
        version: ORIGINAL,
        versions: [],
        snapshots: [],
        zoom: null,
        fullSize: null,
        zoomLoading: false,
        zoomFrame: null,
        cropping: false,
        cropStart: null,
        tool: get().tool === "crop" ? "adjust" : get().tool,
        selectedSpot: null,
        sourceSize: null,
        presetThumbs: {},
      });
      try {
        const opened = await invoke<DevelopOpened>("develop_open", { mediaId });
        if (token !== openToken) return;
        set({
          loading: false,
          settings: opened.settings,
          asShot: opened.asShot,
          presets: opened.presets,
          adapter: opened.adapter,
          decodeMs: opened.decodeMs,
          sourceSize: { width: opened.sourceWidth, height: opened.sourceHeight },
          lensName: opened.lens,
        });
        await Promise.all([render(), loadState(mediaId)]);
        if (get().tool === "filters") void refreshThumbs();
        // Après un moment sur la même photo, le zoom se prépare sans bloquer.
        if (prepareTimer) clearTimeout(prepareTimer);
        prepareTimer = setTimeout(() => {
          if (get().mediaId === mediaId) void ensureFull(mediaId);
        }, ZOOM_PREPARE_DELAY_MS);
        if (get().compare !== "off") await loadBefore();
      } catch (e) {
        if (token === openToken) set({ loading: false, error: toCommandError(e) });
      }
    },

    setLight: (key, value) => {
      const { settings } = get();
      if (!settings) return;
      const v = key === "exposure" ? round(value, 0.01) : Math.round(value);
      change({ ...settings, light: withValue(settings.light, key, v) }, `light.${key}`);
    },

    setColor: (key, value) => {
      const { settings } = get();
      if (!settings) return;
      const v = Math.round(value);
      change({ ...settings, color: withValue(settings.color, key, v) }, `color.${key}`);
    },

    setWhiteBalance: (wb) => applyWhiteBalance(wb, "color.whiteBalance"),

    setCurve: (channel, points) => {
      const { settings } = get();
      if (!settings) return;
      const rounded = points.map(([x, y]) => [Math.round(x), Math.round(y)] as CurvePoint);
      const curve = put(
        settings.curve ?? {},
        channel,
        isIdentityCurve(rounded) ? undefined : rounded,
      );
      change(put(settings, "curve", prune(curve)), `curve.${channel}`);
    },

    setHsl: (kind, hue, value) => {
      const { settings } = get();
      if (!settings) return;
      const set = prune(withValue(settings.hsl?.[kind], hue, Math.round(value)));
      const hsl = put(settings.hsl ?? {}, kind, set);
      change(put(settings, "hsl", prune(hsl)), `hsl.${kind}.${hue}`);
    },

    setLens: (key, value) => {
      const { settings } = get();
      if (!settings) return;
      const v = typeof value === "number" ? Math.round(value) : value;
      const section = put(
        { ...settings.lens } as Record<string, unknown>,
        key,
        v === LENS_DEFAULTS[key] ? undefined : v,
      ) as Partial<LensSettings>;
      change(put(settings, "lens", prune(section)), `lens.${key}`);
    },

    setDetail: (group, key, value) => {
      const { settings } = get();
      if (!settings) return;
      const v = key === "radius" ? Math.round(value * 10) / 10 : Math.round(value);
      const fallback = (DETAIL_DEFAULTS[group] as Record<string, number>)[key];
      const current: Record<string, number> = { ...settings.detail?.[group] };
      const section = put(current, key, v === fallback ? undefined : v);
      const detail = put(
        (settings.detail ?? {}) as Record<string, unknown>,
        group,
        prune(section),
      ) as DevelopSettings["detail"] & object;
      change(put(settings, "detail", prune(detail)), `detail.${group}.${key}`);
    },

    setEffect: (group, key, value) => {
      const { settings } = get();
      if (!settings) return;
      const v = Math.round(value);
      const name = key as string;
      const fallback = (EFFECT_DEFAULTS[group] as Record<string, number>)[name];
      const current: Record<string, number> = { ...settings.effects?.[group] };
      const section = put(current, name, v === fallback ? undefined : v);
      const effects = put(
        (settings.effects ?? {}) as Record<string, unknown>,
        group,
        prune(section),
      ) as DevelopSettings["effects"] & object;
      change(put(settings, "effects", prune(effects)), `effects.${group}.${name}`);
    },

    setTemperature: (kelvin) => {
      const { settings, asShot, presets } = get();
      if (!settings || !asShot) return;
      const { tint } = whitePoint(settings.color?.whiteBalance, asShot, presets);
      get().setWhiteBalance({ mode: "custom", temperature: Math.round(kelvin), tint });
    },

    setTint: (tint) => {
      const { settings, asShot, presets } = get();
      if (!settings || !asShot) return;
      const { temperature } = whitePoint(settings.color?.whiteBalance, asShot, presets);
      get().setWhiteBalance({ mode: "custom", temperature, tint: Math.round(tint) });
    },

    setPicking: (picking) => set({ picking }),

    pickWhite: async (x, y) => {
      set({ picking: false });
      try {
        const wb = await invoke<WhiteBalance>("develop_pick_white", {
          x: Math.round(x),
          y: Math.round(y),
        });
        get().setWhiteBalance(wb);
      } catch (e) {
        set({ error: toCommandError(e) });
      }
    },

    autoLight: async (key) => {
      const { settings } = get();
      if (!settings) return;
      const result = await computeAuto<DevelopSettings>("develop_auto_light", {
        settings,
        key: key ?? null,
      });
      if (result) change(result, key ? `light.${key}.auto` : "light.auto");
    },

    autoWhite: async () => {
      if (!get().settings) return;
      const wb = await computeAuto<WhiteBalance>("develop_auto_white", {});
      if (wb) applyWhiteBalance(wb, "color.whiteBalance.auto");
    },

    toggleClipping: (which) => {
      const { clipping } = get();
      set({ clipping: { ...clipping, [which]: !clipping[which] } });
      void render();
    },

    flush: async () => {
      if (saveTimer) clearTimeout(saveTimer);
      saveTimer = null;
      const label = pendingLabel;
      pendingLabel = null;
      const { settings, mediaId, version } = get();
      if (!label || !settings || mediaId === null) return;
      try {
        await invoke("develop_save", { mediaId, version, settings, label });
        await loadHistory(mediaId);
      } catch (e) {
        set({ error: toCommandError(e) });
      }
    },

    goTo: async (position) => {
      await get().flush();
      const { mediaId, version } = get();
      if (mediaId === null) return;
      try {
        const settings = await invoke<DevelopSettings>("develop_go_to", {
          mediaId,
          version,
          position,
        });
        if (get().mediaId !== mediaId) return;
        set({ settings });
        void render();
        await loadHistory(mediaId);
      } catch (e) {
        set({ error: toCommandError(e) });
      }
    },

    undo: async () => {
      // Un réglage pas encore enregistré devient d'abord une étape, puis est annulé.
      await get().flush();
      const { history } = get();
      if (history && history.current > 0) await get().goTo(history.current - 1);
    },

    redo: async () => {
      await get().flush();
      const { history } = get();
      if (history && history.current < history.steps.length) await get().goTo(history.current + 1);
    },

    setCompare: (mode) => {
      set({ compare: mode });
      if (mode !== "off") void loadBefore();
    },

    setTool: async (tool) => {
      const current = get().tool;
      if (tool === current) return;
      if (current === "crop") get().finishCrop();
      if (tool === "crop") {
        await get().startCrop();
        return;
      }
      set({ tool, selectedSpot: null, picking: false });
      void render();
      if (tool === "filters") {
        await get().loadPresets();
        void refreshThumbs();
      }
    },

    setCopyOpen: (open) => set({ copyOpen: open }),

    copySettings: (controls) => {
      const { settings } = get();
      if (!settings || controls.length === 0) return;
      set({ clipboard: { settings, controls }, copyOpen: false });
    },

    pasteSettings: async (mediaIds) => {
      const { clipboard, mediaId, settings } = get();
      if (!clipboard) return;
      const others = mediaIds.filter((id) => id !== mediaId);
      try {
        if (mediaId !== null && settings && mediaIds.includes(mediaId)) {
          await get().flush();
          const next = await invoke<DevelopSettings>("develop_paste_apply", {
            settings: get().settings ?? settings,
            from: clipboard.settings,
            controls: clipboard.controls,
          });
          if (get().mediaId === mediaId) {
            set({ zoom: null, zoomFrame: null, fullSize: null });
            change(next, "paste");
            await get().flush();
          }
        }
        if (others.length > 0) {
          await invoke<number>("develop_paste", {
            mediaIds: others,
            from: clipboard.settings,
            controls: clipboard.controls,
          });
        }
        set({ notice: i18next.t("develop.paste.done", { count: mediaIds.length }) });
        if (noticeTimer) clearTimeout(noticeTimer);
        noticeTimer = setTimeout(() => set({ notice: null }), NOTICE_MS);
      } catch (e) {
        set({ error: toCommandError(e) });
      }
    },

    loadPresets: async () => {
      try {
        const presetGroups = await invoke<PresetGroup[]>("presets_list");
        set({ presetGroups });
      } catch (e) {
        set({ error: toCommandError(e) });
      }
    },

    applyPreset: async (id, amount = 100) => {
      const { settings, mediaId } = get();
      if (!settings) return;
      await get().flush();
      try {
        const next = await invoke<DevelopSettings>("develop_preset_apply", {
          settings: get().settings ?? settings,
          id,
          amount,
        });
        if (get().mediaId !== mediaId) return;
        change(next, lookLabel(id));
      } catch (e) {
        set({ error: toCommandError(e) });
      }
    },

    setPresetAmount: async (amount) => {
      const { settings } = get();
      const look = settings?.look;
      if (!settings || !look) return;
      try {
        const next = await invoke<DevelopSettings>("develop_preset_apply", {
          settings,
          id: look.id,
          amount: Math.round(amount),
        });
        change(next, "look.amount");
      } catch (e) {
        set({ error: toCommandError(e) });
      }
    },

    createPreset: async (name, controls) => {
      const { settings } = get();
      if (!settings) return false;
      try {
        await invoke("presets_create", { name, settings, controls });
        await get().loadPresets();
        void refreshThumbs();
        return true;
      } catch (e) {
        set({ error: toCommandError(e) });
        return false;
      }
    },

    renamePreset: async (id, name) => {
      try {
        await invoke("presets_rename", { id, name });
        await get().loadPresets();
        return true;
      } catch (e) {
        set({ error: toCommandError(e) });
        return false;
      }
    },

    deletePreset: async (id) => {
      try {
        await invoke("presets_delete", { id });
        await get().loadPresets();
      } catch (e) {
        set({ error: toCommandError(e) });
      }
    },

    addSpot: async (x, y) => {
      const { settings, spotBrush } = get();
      if (!settings) return;
      const source = await computeAuto<[number, number]>("develop_spot_source", {
        x,
        y,
        radius: spotBrush.radius,
        others: settings.spots ?? [],
      });
      const now = get().settings;
      if (!source || !now) return;
      const spot = sparseSpot({
        x,
        y,
        radius: spotBrush.radius,
        sourceX: source[0],
        sourceY: source[1],
        feather: spotBrush.feather,
        mode: spotBrush.mode,
      });
      const spots = [...(now.spots ?? []), spot];
      await changeSpots(spots, "spots.add");
      set({ selectedSpot: spots.length - 1 });
    },

    moveSpot: async (index, part, x, y) => {
      const spots = [...(get().settings?.spots ?? [])];
      const spot = spots[index];
      if (!spot) return;
      const [cx, cy] = [Math.min(1, Math.max(0, x)), Math.min(1, Math.max(0, y))];
      spots[index] = sparseSpot(
        part === "target" ? { ...spot, x: cx, y: cy } : { ...spot, sourceX: cx, sourceY: cy },
      );
      await changeSpots(spots, part === "target" ? "spots.move" : "spots.source");
    },

    setSpotBrush: (patch) => {
      const { spotBrush, selectedSpot, settings } = get();
      set({ spotBrush: { ...spotBrush, ...patch } });
      const spots = settings?.spots ?? [];
      const spot = selectedSpot === null ? undefined : spots[selectedSpot];
      if (!settings || !spot || selectedSpot === null) return;
      const next = [...spots];
      next[selectedSpot] = sparseSpot({ ...spot, ...patch });
      const key = Object.keys(patch)[0] ?? "radius";
      change(put(settings, "spots", next), `spots.${key}`);
    },

    selectSpot: (index) => {
      const spot = index === null ? undefined : get().settings?.spots?.[index];
      // Le pinceau reprend les réglages de la correction choisie.
      if (spot && index !== null) {
        set({
          selectedSpot: index,
          spotBrush: {
            radius: spot.radius,
            feather: spot.feather ?? SPOT_FEATHER_DEFAULT,
            mode: spot.mode ?? "heal",
          },
        });
      } else {
        set({ selectedSpot: null });
      }
    },

    removeSpot: async (index) => {
      const spots = (get().settings?.spots ?? []).filter((_, i) => i !== index);
      set({ selectedSpot: null });
      await changeSpots(spots, "spots.remove");
    },

    clearSpots: async () => {
      set({ selectedSpot: null });
      await changeSpots([], "spots.clear");
    },

    setShowDust: (show) => {
      set({ showDust: show });
      void render();
    },

    startCrop: async () => {
      if (!get().settings || get().cropping) return;
      await get().flush();
      set({
        tool: "crop",
        selectedSpot: null,
        picking: false,
        cropping: true,
        cropStart: get().history?.current ?? 0,
        zoom: null,
        zoomFrame: null,
        compare: "off",
      });
      void render();
    },

    finishCrop: () => {
      if (!get().cropping) return;
      set({ cropping: false, cropStart: null, tool: "adjust" });
      void render();
    },

    cancelCrop: async () => {
      const { cropStart, history } = get();
      set({ cropping: false, cropStart: null, tool: "adjust" });
      await get().flush();
      if (cropStart !== null && history && get().history?.current !== cropStart) {
        await get().goTo(cropStart);
      } else {
        void render();
      }
    },

    setCrop: async (crop) => {
      const geometry = get().settings?.geometry ?? {};
      const r = (v: number) => Math.round(Math.min(1, Math.max(0, v)) * 10000) / 10000;
      const next = await constrained({
        ...geometry,
        crop: {
          left: r(crop.left),
          top: r(crop.top),
          right: r(crop.right),
          bottom: r(crop.bottom),
        },
      });
      changeGeometry(next, "geometry.crop");
    },

    setAngle: async (angle, label = "geometry.angle") => {
      const geometry = get().settings?.geometry ?? {};
      const a = Math.round(Math.min(45, Math.max(-45, angle)) * 10) / 10;
      // Le cadre part de l'image entière (ou du recadrage actuel) puis rétrécit.
      const next = await constrained({ ...geometry, angle: a });
      changeGeometry(next, label);
    },

    autoStraighten: async () => {
      const { settings } = get();
      if (!settings) return;
      const angle = await computeAuto<number>("develop_auto_straighten", {
        geometry: settings.geometry ?? {},
      });
      if (angle !== null) await get().setAngle(angle, "geometry.auto");
    },

    rotateQuarter: (direction) => {
      const g = get().settings?.geometry ?? {};
      const c = g.crop ?? FULL_CROP;
      // Le cadre tourne avec l'image.
      const crop =
        direction === 1
          ? { left: 1 - c.bottom, top: c.left, right: 1 - c.top, bottom: c.right }
          : { left: c.top, top: 1 - c.right, right: c.bottom, bottom: 1 - c.left };
      const quarter = ((((g.quarter ?? 0) + direction) % 4) + 4) % 4;
      changeGeometry({ ...g, quarter, crop }, "geometry.quarter");
    },

    flip: (axis) => {
      const g = get().settings?.geometry ?? {};
      const c = g.crop ?? FULL_CROP;
      const next =
        axis === "h"
          ? { ...g, flipH: !g.flipH, crop: { ...c, left: 1 - c.right, right: 1 - c.left } }
          : { ...g, flipV: !g.flipV, crop: { ...c, top: 1 - c.bottom, bottom: 1 - c.top } };
      // Un miroir change le sens du redressement.
      next.angle = -(g.angle ?? 0);
      changeGeometry(next, "geometry.flip");
    },

    setAspect: async (aspect, portrait) => {
      const { settings, frame } = get();
      if (!settings || !frame) return;
      const g = settings.geometry ?? {};
      const c = g.crop ?? FULL_CROP;
      // Proportions du canevas (l'outil l'affiche en entier).
      const canvas = frame.width / frame.height;
      const base = aspectRatio(aspect, canvas);
      let crop = c;
      if (base !== null) {
        const tall = portrait ?? (c.bottom - c.top) * (1 / canvas) > c.right - c.left;
        const want = tall ? 1 / base : base;
        // Plus grand cadre de ce ratio centré sur l'actuel, dans le canevas.
        const cx = (c.left + c.right) / 2;
        const cy = (c.top + c.bottom) / 2;
        let w = c.right - c.left;
        let h = (w * canvas) / want;
        if (h > c.bottom - c.top) {
          h = c.bottom - c.top;
          w = (h * want) / canvas;
        }
        const half = (v: number, lo: number) => Math.min(v / 2, lo, 1 - lo);
        const hw = half(w, cx);
        const hh = half(h, cy);
        const k = Math.min(hw / (w / 2), hh / (h / 2));
        crop = {
          left: cx - (w / 2) * k,
          right: cx + (w / 2) * k,
          top: cy - (h / 2) * k,
          bottom: cy + (h / 2) * k,
        };
      }
      const next = await constrained({ ...g, aspect, crop });
      changeGeometry(next, "geometry.aspect");
    },

    resetGeometry: () => changeGeometry({}, "geometry.reset"),

    zoomAt: async (fx, fy) => {
      const { mediaId } = get();
      if (mediaId === null || get().zoomLoading || get().cropping) return;
      if (!get().fullSize) {
        set({ zoomLoading: true });
        const ready = await ensureFull(mediaId);
        if (get().mediaId === mediaId) set({ zoomLoading: false });
        if (!ready) return;
      }
      const { fullSize, viewport } = get();
      if (!fullSize) return;
      set({
        zoom: clampZoom({
          left: fx * fullSize.width - viewport.width / 2,
          top: fy * fullSize.height - viewport.height / 2,
        }),
      });
      await renderZoom();
    },

    panZoom: (dx, dy) => {
      const { zoom } = get();
      if (!zoom) return;
      set({ zoom: clampZoom({ left: zoom.left - dx, top: zoom.top - dy }) });
      void renderZoom();
    },

    exitZoom: () => set({ zoom: null, zoomFrame: null }),

    setViewport: (width, height) => {
      const v = get().viewport;
      if (Math.round(v.width) === Math.round(width) && Math.round(v.height) === Math.round(height))
        return;
      set({ viewport: { width, height } });
      const { zoom } = get();
      if (zoom) {
        set({ zoom: clampZoom(zoom) });
        void renderZoom();
      }
    },

    selectVersion: async (name) => {
      const { mediaId } = get();
      if (mediaId === null || name === get().version) return;
      await get().flush();
      set({ version: name, history: null, snapshots: [] });
      await loadState(mediaId);
      void render();
    },

    createVersion: async (name) => {
      await get().flush();
      if (await run("develop_version_create", { from: get().version, name })) {
        await get().selectVersion(name.trim());
      }
    },

    renameVersion: async (from, to) => {
      const { mediaId } = get();
      if (mediaId === null || !(await run("develop_version_rename", { from, to }))) return false;
      if (get().version === from) set({ version: to.trim() });
      await loadState(mediaId);
      return true;
    },

    deleteVersion: async (name) => {
      const { mediaId } = get();
      if (mediaId === null) return;
      await get().flush();
      if (!(await run("develop_version_delete", { name }))) return;
      if (get().version === name) {
        set({ version: ORIGINAL });
        await loadState(mediaId);
        void render();
      } else {
        await loadState(mediaId);
      }
    },

    createSnapshot: async (name) => {
      const { mediaId } = get();
      if (mediaId === null) return;
      // L'instantané garde ce qui est affiché : le réglage en attente d'abord.
      await get().flush();
      if (await run("develop_snapshot_create", { version: get().version, name })) {
        await loadState(mediaId);
      }
    },

    applySnapshot: async (name) => {
      const { mediaId } = get();
      if (mediaId === null) return;
      await get().flush();
      const { version } = get();
      try {
        const settings = await invoke<DevelopSettings>("develop_snapshot_apply", {
          mediaId,
          version,
          name,
        });
        if (get().mediaId !== mediaId || get().version !== version) return;
        set({ settings });
        void render();
        await loadHistory(mediaId);
      } catch (e) {
        set({ error: toCommandError(e) });
      }
    },

    renameSnapshot: async (from, to) => {
      const { mediaId, version } = get();
      if (mediaId === null) return false;
      if (!(await run("develop_snapshot_rename", { version, from, to }))) return false;
      await loadState(mediaId);
      return true;
    },

    deleteSnapshot: async (name) => {
      const { mediaId, version } = get();
      if (mediaId === null) return;
      if (await run("develop_snapshot_delete", { version, name })) await loadState(mediaId);
    },

    close: async () => {
      await get().flush();
      if (prepareTimer) clearTimeout(prepareTimer);
      openToken++;
      set({
        mediaId: null,
        settings: null,
        frame: null,
        loading: false,
        error: null,
        picking: false,
        history: null,
        beforeFrame: null,
        compare: "off",
        version: ORIGINAL,
        versions: [],
        snapshots: [],
        zoom: null,
        fullSize: null,
        zoomFrame: null,
        cropping: false,
        cropStart: null,
        tool: get().tool === "crop" ? "adjust" : get().tool,
        selectedSpot: null,
        sourceSize: null,
      });
      await invoke("develop_close").catch(() => undefined);
    },
  };
});

/** Annuler possible : une étape avant la position courante, ou un réglage en attente. */
export function canUndo(s: Pick<DevelopState, "history" | "settings">): boolean {
  return s.settings !== null && ((s.history?.current ?? 0) > 0 || pendingLabel !== null);
}

export function canRedo(s: Pick<DevelopState, "history" | "settings">): boolean {
  return s.settings !== null && !!s.history && s.history.current < s.history.steps.length;
}
