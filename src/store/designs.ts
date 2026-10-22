import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";
import { type CommandError, toCommandError } from "../api/core";
import { DESIGN_VERSION, type Design, type DesignText, layoutOf, maxPhotos } from "../ui/design";

/** Miroir de `export::design::Saved`. */
export interface SavedDesign {
  id: number;
  name: string;
  document: Design;
  updatedAt: string;
}

/** Miroir de `fonts::FontList` : polices livrées, puis installées. */
export interface FontList {
  bundled: string[];
  system: string[];
}

/** Composition ouverte dans l'éditeur. */
export interface Draft {
  id: number | null;
  name: string;
  document: Design;
}

interface DesignsState {
  saved: SavedDesign[];
  fonts: FontList | null;
  draft: Draft | null;
  /** Modifiée depuis l'ouverture ou le dernier enregistrement. */
  dirty: boolean;
  /** Texte sélectionné dans l'éditeur (indice), ou le logo. */
  selected: number | "logo" | null;
  busy: boolean;
  /** Dernier fichier exporté. */
  exported: string | null;
  error: CommandError | null;
  load: () => Promise<void>;
  loadFonts: () => Promise<void>;
  /**
   * Nouvelle composition au format `width` × `height` d'un modèle ; un
   * Polaroid reçoit `caption` comme légende de départ.
   */
  create: (
    template: string,
    width: number,
    height: number,
    items: number[],
    name: string,
    caption?: string,
  ) => Promise<void>;
  open: (saved: SavedDesign) => void;
  close: () => void;
  update: (patch: Partial<Design>) => void;
  rename: (name: string) => void;
  select: (selected: DesignsState["selected"]) => void;
  /** Ajoute des photos (jusqu'à quatre), centrées sur leur visage. */
  addPhotos: (items: number[]) => Promise<void>;
  removePhoto: (index: number) => void;
  save: () => Promise<number | null>;
  /** Enregistre, puis exporte dans le dossier de destination de Livrer. */
  exportDraft: () => Promise<void>;
  remove: (id: number) => Promise<void>;
}

async function focusOf(items: number[]) {
  return Promise.all(
    items.map(async (item) => ({
      item,
      focus: await invoke<[number, number]>("design_focus", { item }).catch(
        (): [number, number] => [0.5, 0.5],
      ),
    })),
  );
}

export const useDesigns = create<DesignsState>()((set, get) => ({
  saved: [],
  fonts: null,
  draft: null,
  dirty: false,
  selected: null,
  busy: false,
  exported: null,
  error: null,

  load: async () => {
    try {
      set({ saved: await invoke<SavedDesign[]>("design_list") });
    } catch (e) {
      set({ error: toCommandError(e) });
    }
  },

  loadFonts: async () => {
    if (get().fonts) return;
    try {
      set({ fonts: await invoke<FontList>("fonts_list") });
    } catch (e) {
      set({ error: toCommandError(e) });
    }
  },

  create: async (template, width, height, items, name, caption = "") => {
    const layout = layoutOf(template);
    const photos = await focusOf(items.slice(0, maxPhotos(layout)));
    if (photos.length === 0) return;
    // Polaroid : fond crème, légende manuscrite dans le bord du bas.
    const texts: DesignText[] =
      layout === "polaroid"
        ? [
            {
              text: caption,
              x: 0.5,
              y: 0.84,
              size: 6,
              font: "Caveat",
              weight: 400,
              color: "#2a2a2a",
              align: "center",
            },
          ]
        : [];
    set({
      exported: null,
      selected: null,
      dirty: true,
      draft: {
        id: null,
        name,
        document: {
          version: DESIGN_VERSION,
          template,
          layout,
          width,
          height,
          photos,
          background: layout === "polaroid" ? "#f6f3ea" : "#ffffff",
          margin: layout === "strip" ? 4 : photos.length > 1 ? 3 : 0,
          frame: null,
          texts,
          logo: null,
        },
      },
    });
  },

  open: (saved) =>
    set({
      draft: { id: saved.id, name: saved.name, document: saved.document },
      selected: null,
      exported: null,
      dirty: false,
    }),

  close: () => set({ draft: null, selected: null }),

  update: (patch) => {
    const { draft } = get();
    if (draft) set({ draft: { ...draft, document: { ...draft.document, ...patch } }, dirty: true });
  },

  rename: (name) => {
    const { draft } = get();
    if (draft) set({ draft: { ...draft, name }, dirty: true });
  },

  select: (selected) => set({ selected }),

  addPhotos: async (items) => {
    const { draft } = get();
    if (!draft) return;
    const present = new Set(draft.document.photos.map((p) => p.item));
    const room = maxPhotos(draft.document.layout) - draft.document.photos.length;
    const added = await focusOf(items.filter((i) => !present.has(i)).slice(0, room));
    const current = get().draft;
    if (current && added.length > 0) {
      get().update({ photos: [...current.document.photos, ...added] });
    }
  },

  removePhoto: (index) => {
    const { draft } = get();
    if (!draft || draft.document.photos.length <= 1) return;
    get().update({ photos: draft.document.photos.filter((_, i) => i !== index) });
  },

  save: async () => {
    const { draft } = get();
    if (!draft) return null;
    set({ busy: true });
    try {
      const id = await invoke<number>("design_save", {
        id: draft.id,
        name: draft.name,
        document: draft.document,
      });
      const current = get().draft;
      if (current) set({ draft: { ...current, id }, dirty: false });
      await get().load();
      return id;
    } catch (e) {
      set({ error: toCommandError(e) });
      return null;
    } finally {
      set({ busy: false });
    }
  },

  exportDraft: async () => {
    const id = await get().save();
    if (id === null) return;
    set({ busy: true, exported: null });
    try {
      set({ exported: await invoke<string>("design_export", { id }) });
    } catch (e) {
      set({ error: toCommandError(e) });
    } finally {
      set({ busy: false });
    }
  },

  remove: async (id) => {
    try {
      await invoke("design_delete", { id });
      if (get().draft?.id === id) set({ draft: null });
      await get().load();
    } catch (e) {
      set({ error: toCommandError(e) });
    }
  },
}));
