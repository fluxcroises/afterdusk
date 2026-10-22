import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";
import { type CommandError, toCommandError } from "../api/core";

/** Miroir de `export::templates::Template`. */
export interface DeliveryTemplate {
  id: string;
  group: "social" | "print" | "web";
  width: number;
  height: number;
}

/** Miroir de `export::templates::Framing` : gauche, haut, droite, bas (0 à 1). */
export interface Framing {
  template: string;
  rect: [number, number, number, number];
  custom: boolean;
  width: number;
  height: number;
  sharpEnough: boolean;
}

export const GROUPS: DeliveryTemplate["group"][] = ["social", "print", "web"];

/** Preset qui donne le format, les couleurs et les métadonnées d'un usage. */
export const GROUP_PRESET: Record<DeliveryTemplate["group"], string> = {
  social: "instagram",
  print: "full",
  web: "web",
};

const CHOSEN_KEY = "afterdusk.export.templates";

function readChosen(): string[] {
  try {
    const raw = window.localStorage.getItem(CHOSEN_KEY);
    const parsed: unknown = raw ? JSON.parse(raw) : null;
    return Array.isArray(parsed) ? parsed.filter((v) => typeof v === "string") : [];
  } catch {
    return [];
  }
}

function writeChosen(ids: string[]) {
  try {
    window.localStorage.setItem(CHOSEN_KEY, JSON.stringify(ids));
  } catch {
    // Préférence de confort : sans stockage, elle vaut pour la session.
  }
}

interface TemplatesState {
  /** Livrer affiche la grille des photos ou les modèles. */
  view: "photos" | "templates";
  templates: DeliveryTemplate[];
  /** Photo dont les cadrages sont chargés. */
  item: number | null;
  framings: Framing[];
  loading: boolean;
  /** Modèles cochés pour le prochain export. */
  chosen: string[];
  /** Modèle dont le cadrage est ouvert. */
  editing: string | null;
  error: CommandError | null;
  setView: (view: TemplatesState["view"]) => void;
  /** Modèles et cadrages de la photo `item`. */
  load: (item: number | null) => Promise<void>;
  toggle: (id: string) => void;
  choose: (id: string) => void;
  setEditing: (id: string | null) => void;
  save: (template: string, rect: Framing["rect"]) => Promise<void>;
  reset: (template: string) => Promise<void>;
}

export const useTemplates = create<TemplatesState>()((set, get) => ({
  view: "photos",
  templates: [],
  item: null,
  framings: [],
  loading: false,
  chosen: readChosen(),
  editing: null,
  error: null,

  setView: (view) => set({ view }),

  load: async (item) => {
    set({ item, loading: item !== null });
    try {
      const templates =
        get().templates.length > 0
          ? get().templates
          : await invoke<DeliveryTemplate[]>("delivery_templates");
      const ids = templates.map((t) => t.id);
      set({ templates, chosen: get().chosen.filter((id) => ids.includes(id)) });
      if (item === null) {
        set({ framings: [] });
        return;
      }
      const framings = await invoke<Framing[]>("delivery_framings", { item });
      // Une autre photo a pu être choisie entre-temps.
      if (get().item === item) set({ framings });
    } catch (e) {
      set({ error: toCommandError(e), framings: [] });
    } finally {
      if (get().item === item) set({ loading: false });
    }
  },

  toggle: (id) => {
    const { chosen } = get();
    const next = chosen.includes(id) ? chosen.filter((c) => c !== id) : [...chosen, id];
    writeChosen(next);
    set({ chosen: next });
  },

  choose: (id) => {
    if (!get().chosen.includes(id)) get().toggle(id);
  },

  setEditing: (editing) => set({ editing }),

  save: async (template, rect) => {
    const { item } = get();
    if (item === null) return;
    try {
      await invoke("delivery_framing_set", { item, template, rect });
      await get().load(item);
    } catch (e) {
      set({ error: toCommandError(e) });
    }
  },

  reset: async (template) => {
    const { item } = get();
    if (item === null) return;
    try {
      await invoke("delivery_framing_reset", { item, template });
      await get().load(item);
    } catch (e) {
      set({ error: toCommandError(e) });
    }
  },
}));
