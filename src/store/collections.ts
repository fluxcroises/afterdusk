import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";
import { toCommandError } from "../api/core";
import { type MediaFilter, useLibrary } from "./library";

/** Miroir de `library::collections::Collection`. */
export interface Collection {
  id: number;
  name: string;
  kind: "manual" | "smart";
  /** Albums intelligents seulement. */
  filter: MediaFilter | null;
  count: number;
}

/** Filtre relu : les champs vides (`null`, liste vide) ne filtrent pas, ils disparaissent. */
export function cleanFilter(filter: MediaFilter | null): MediaFilter {
  return Object.fromEntries(
    Object.entries(filter ?? {}).filter(
      ([, v]) => v !== null && v !== undefined && !(Array.isArray(v) && v.length === 0),
    ),
  ) as MediaFilter;
}

/** Filtre de la barre, sans la collection affichée : ce qu'enregistre un album intelligent. */
export function barFilter(filter: MediaFilter): MediaFilter {
  const rest = { ...filter };
  delete rest.collectionId;
  return rest;
}

/** Deux filtres équivalents (ordre des clés et listes vides ignorés). */
export function sameFilter(a: MediaFilter, b: MediaFilter): boolean {
  const norm = (f: MediaFilter) =>
    JSON.stringify(
      Object.entries(barFilter(f))
        .filter(([, v]) => v !== undefined && !(Array.isArray(v) && v.length === 0))
        .sort(([x], [y]) => x.localeCompare(y)),
    );
  return norm(a) === norm(b);
}

interface CollectionsState {
  collections: Collection[];
  /** Album intelligent ouvert en dernier, mis en avant tant qu'aucun autre choix n'est fait. */
  activeSmartId: number | null;
  load: () => Promise<void>;
  open: (collection: Collection) => Promise<void>;
  /** Collection manuelle vide, ou album intelligent du filtre courant. */
  create: (name: string, kind: Collection["kind"]) => Promise<void>;
  rename: (id: number, name: string) => Promise<void>;
  /** Remplace le filtre de l'album intelligent par celui de la barre. */
  updateFilter: (id: number) => Promise<void>;
  remove: (id: number) => Promise<void>;
  /** Ajoute la sélection de la grille à une collection manuelle. */
  addSelection: (id: number) => Promise<void>;
  /** Retire la sélection de la collection manuelle affichée. */
  removeSelection: () => Promise<void>;
}

export const useCollections = create<CollectionsState>()((set, get) => {
  async function guard(run: () => Promise<void>) {
    try {
      await run();
    } catch (e) {
      useLibrary.setState({ error: toCommandError(e) });
    }
  }

  return {
    collections: [],
    activeSmartId: null,

    load: () =>
      guard(async () => {
        const libraryId = useLibrary.getState().libraryId;
        if (libraryId === null) {
          set({ collections: [] });
          return;
        }
        const collections = await invoke<Collection[] | null>("collections_list", { libraryId });
        set({
          collections: (collections ?? []).map((c) => ({
            ...c,
            filter: c.kind === "smart" ? cleanFilter(c.filter) : null,
          })),
        });
      }),

    open: (c) =>
      guard(async () => {
        const library = useLibrary.getState();
        if (c.kind === "manual") {
          set({ activeSmartId: null });
          await library.showCollection(c.id);
        } else {
          set({ activeSmartId: c.id });
          await library.applyFilter(c.filter ?? {});
        }
      }),

    create: (name, kind) =>
      guard(async () => {
        const { libraryId, filter } = useLibrary.getState();
        if (libraryId === null) return;
        const id = await invoke<number>("collection_create", {
          libraryId,
          name,
          filter: kind === "smart" ? barFilter(filter) : null,
        });
        await get().load();
        if (kind === "smart") set({ activeSmartId: id });
      }),

    rename: (id, name) =>
      guard(async () => {
        await invoke("collection_rename", { id, name });
        await get().load();
      }),

    updateFilter: (id) =>
      guard(async () => {
        await invoke("collection_set_filter", {
          id,
          filter: barFilter(useLibrary.getState().filter),
        });
        await get().load();
      }),

    remove: (id) =>
      guard(async () => {
        await invoke("collection_delete", { id });
        const library = useLibrary.getState();
        if (library.collectionId === id) await library.showAlbum(null);
        if (get().activeSmartId === id) set({ activeSmartId: null });
        await get().load();
      }),

    addSelection: (id) =>
      guard(async () => {
        // Les copies virtuelles (identifiants négatifs) n'ont pas de ligne propre.
        const mediaIds = useLibrary
          .getState()
          .targetIds()
          .filter((m) => m > 0);
        if (mediaIds.length === 0) return;
        await invoke<number>("collection_add_media", { id, mediaIds });
        await get().load();
      }),

    removeSelection: () =>
      guard(async () => {
        const library = useLibrary.getState();
        const id = library.collectionId;
        const mediaIds = library.targetIds().filter((m) => m > 0);
        if (id === null || mediaIds.length === 0) return;
        await invoke<number>("collection_remove_media", { id, mediaIds });
        await library.reloadMedia();
        await get().load();
      }),
  };
});
