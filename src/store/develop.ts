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
  /** Auto de la section Lumière, ou d'un seul curseur. */
  autoLight: (key?: LightKey) => Promise<void>;
  autoWhite: () => Promise<void>;
  /** Revient à une étape de l'historique (0 : l'original). */
  goTo: (position: number) => Promise<void>;
  undo: () => Promise<void>;
  redo: () => Promise<void>;
  setCompare: (mode: CompareMode) => void;
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
  /** Décode l'original en pleine définition (une fois par photo). */
  function ensureFull(mediaId: number): Promise<boolean> {
    if (get().fullSize) return Promise.resolve(true);
    if (fullDecode?.mediaId === mediaId) return fullDecode.promise;
    const promise = invoke<{ width: number; height: number }>("develop_zoom_open")
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
        const { settings, mediaId, clipping, zoom, viewport } = get();
        if (!settings || mediaId === null || !zoom) return;
        const buffer = await invoke<ArrayBuffer>("develop_zoom_render", {
          settings,
          clipping,
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
        const { settings, mediaId, clipping } = get();
        if (!settings || mediaId === null) return;
        const started = performance.now();
        const buffer = await invoke<ArrayBuffer>("develop_render", { settings, clipping });
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
        });
        await Promise.all([render(), loadState(mediaId)]);
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

    zoomAt: async (fx, fy) => {
      const { mediaId } = get();
      if (mediaId === null || get().zoomLoading) return;
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
