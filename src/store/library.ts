import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { type CommandError, toCommandError } from "../api/core";

export interface MediaSummary {
  id: number;
  albumId: number;
  fileName: string;
  capturedAt: string | null;
  rating: number;
  flag: number;
  color: string | null;
  width: number | null;
  height: number | null;
  orientation: number;
  hasPreview: boolean;
  /** Date de mise à la corbeille (vue Corbeille seulement). */
  trashedAt: string | null;
}

export interface TrashReport {
  done: number;
  failed: number;
}

export interface TrashSummary {
  count: number;
  retentionDays: number;
}

/** Dernière opération sur la corbeille, annonçable et, pour un envoi, annulable. */
export interface TrashNotice {
  kind: "trashed" | "restored" | "purged";
  count: number;
}

const DAY_MS = 24 * 60 * 60 * 1000;

/** Jours avant la suppression définitive d'une photo de la corbeille. */
export function daysLeft(
  trashedAt: string | null,
  retentionDays: number,
  now = Date.now(),
): number {
  if (!trashedAt) return retentionDays;
  const elapsed = (now - new Date(trashedAt).getTime()) / DAY_MS;
  return Math.max(0, Math.ceil(retentionDays - elapsed));
}

export interface AlbumSummary {
  id: number;
  name: string;
  folder: string;
  startDate: string | null;
  endDate: string | null;
  count: number;
  coverMediaId: number | null;
  coverChosen: boolean;
}

export interface MediaDetails {
  id: number;
  fileName: string;
  originalName: string;
  path: string;
  capturedAt: string | null;
  utcOffset: string | null;
  width: number | null;
  height: number | null;
  camera: string | null;
  lens: string | null;
  focalLength: number | null;
  focalLength35mm: number | null;
  aperture: number | null;
  shutter: number | null;
  iso: number | null;
  exposureCompensation: number | null;
  keywords: string[];
}

export type ColorName = "red" | "yellow" | "green" | "blue" | "purple";
/** -1 rejetée, 0 sans drapeau, 1 retenue. */
export type FlagValue = -1 | 0 | 1;

const AUTO_ADVANCE_KEY = "afterdusk.autoAdvance";

function readAutoAdvance(): boolean {
  try {
    return window.localStorage.getItem(AUTO_ADVANCE_KEY) === "true";
  } catch {
    return false;
  }
}

export const libraryApi = {
  mediaList: (libraryId: number, albumId: number | null, trashed = false) =>
    invoke<MediaSummary[]>("media_list", { libraryId, albumId, trashed }),
  trash: (ids: number[]) => invoke<TrashReport>("media_trash", { ids }),
  restore: (ids: number[]) => invoke<TrashReport>("media_restore", { ids }),
  emptyTrash: (libraryId: number, ids: number[] | null) =>
    invoke<TrashReport>("trash_empty", { libraryId, ids }),
  trashSummary: (libraryId: number) => invoke<TrashSummary>("trash_summary", { libraryId }),
  setRetention: (days: number) => invoke<number>("trash_retention_set", { days }),
  albumsList: (libraryId: number) => invoke<AlbumSummary[]>("albums_list", { libraryId }),
  mediaDetails: (mediaId: number) => invoke<MediaDetails | null>("media_details", { mediaId }),
  setRating: (ids: number[], rating: number) => invoke<number>("media_set_rating", { ids, rating }),
  setFlag: (ids: number[], flag: FlagValue) => invoke<number>("media_set_flag", { ids, flag }),
  setCover: (albumId: number, mediaId: number) =>
    invoke<null>("album_set_cover", { albumId, mediaId }),
  setColor: (ids: number[], color: ColorName | null) =>
    invoke<number>("media_set_color", { ids, color }),
};

type Marks = Pick<MediaSummary, "rating" | "flag" | "color">;

export type LibraryView = "grid" | "loupe" | "compare";
export type SelectMode = "replace" | "toggle" | "range";

/** Comparaison : 2 à 4 photos de la sélection, ou la photo active et la suivante. */
export const MAX_COMPARE = 4;

export function compareIds(s: {
  media: MediaSummary[];
  selectedId: number | null;
  selectedIds: number[];
}): number[] {
  if (s.selectedIds.length >= 2) {
    // Dans l'ordre de la grille, pas dans l'ordre des clics.
    const chosen = new Set(s.selectedIds);
    return s.media
      .filter((m) => chosen.has(m.id))
      .slice(0, MAX_COMPARE)
      .map((m) => m.id);
  }
  const index = s.media.findIndex((m) => m.id === s.selectedId);
  if (index < 0) return [];
  const next = s.media[index + 1] ?? s.media[index - 1];
  const current = s.media[index];
  if (!current) return [];
  return next ? [current.id, next.id] : [current.id];
}

interface LibraryState {
  libraryId: number | null;
  albums: AlbumSummary[];
  /** Album affiché ; `null` pour toutes les photos. */
  albumId: number | null;
  /** Vue Corbeille : les photos jetées, à restaurer ou à supprimer. */
  trash: boolean;
  trashCount: number;
  retentionDays: number;
  notice: TrashNotice | null;
  /** Dernier envoi dans la corbeille, que Ctrl+Z restaure (l'annonce, elle, s'efface). */
  undoIds: number[] | null;
  media: MediaSummary[];
  /** Nombre total de photos de la bibliothèque. */
  total: number;
  /** Photo active : fiche d'informations, visionneuse, cible du clavier. */
  selectedId: number | null;
  /** Sélection (Ctrl+clic, Maj+clic) ; contient toujours la photo active. */
  selectedIds: number[];
  details: MediaDetails | null;
  /** Incrémenté à chaque aperçu régénéré, pour rafraîchir l'image. */
  versions: Record<number, number>;
  error: CommandError | null;
  /** Passer à la photo suivante après une note ou un drapeau (option). */
  autoAdvance: boolean;
  /** Grille, visionneuse (une photo) ou comparaison (2 à 4). */
  view: LibraryView;
  /** Zoom 100 % de la visionneuse : point visé, en fraction de l'image ; `null` = ajusté. */
  zoom: { fx: number; fy: number } | null;
  /** Instant de la dernière navigation, pour mesurer l'affichage de la suivante. */
  navigatedAt: number;

  load: (libraryId: number | null) => Promise<void>;
  showAlbum: (albumId: number | null) => Promise<void>;
  select: (mediaId: number, mode?: SelectMode) => Promise<void>;
  /** Change la photo active sans toucher à la sélection (panneaux de comparaison). */
  activate: (mediaId: number) => Promise<void>;
  /** `at` : horodatage de la frappe, pour mesurer l'affichage de la suivante. */
  move: (delta: number, at?: number) => void;
  previewReady: (mediaId: number) => void;
  setRating: (rating: number) => Promise<void>;
  setFlag: (flag: FlagValue) => Promise<void>;
  /** Pose la couleur, ou la retire si la photo l'a déjà (comme Lightroom). */
  toggleColor: (color: ColorName) => Promise<void>;
  setAutoAdvance: (value: boolean) => void;
  setView: (view: LibraryView) => void;
  setZoom: (zoom: { fx: number; fy: number } | null) => void;
  /** La photo devient la couverture de son album. */
  setAsCover: (mediaId: number) => Promise<void>;
  showTrash: () => Promise<void>;
  /** Photos visées par une commande : la sélection dans la grille, sinon la photo active. */
  targetIds: () => number[];
  /** Envoie des photos dans la corbeille (la sélection par défaut). */
  moveToTrash: (ids?: number[]) => Promise<void>;
  /** Les photos rejetées de la vue affichée. */
  rejectedIds: () => number[];
  restore: (ids?: number[]) => Promise<void>;
  /** Supprime définitivement des photos de la corbeille, ou toute la corbeille. */
  emptyTrash: (ids?: number[]) => Promise<void>;
  /** Annule le dernier envoi dans la corbeille. */
  undoTrash: () => Promise<void>;
  setRetentionDays: (days: number) => Promise<void>;
}

export const useLibrary = create<LibraryState>((set, get) => {
  /**
   * Marque la photo sélectionnée : l'écran change tout de suite, la base suit ;
   * en cas de refus du cœur, l'ancienne valeur est rétablie.
   */
  async function mark(
    patch: Partial<Marks>,
    send: (ids: number[]) => Promise<number>,
    advance: boolean,
  ): Promise<void> {
    const { selectedId, selectedIds, view, media, trash } = get();
    if (selectedId === null || trash) return;
    // Dans la grille, toute la sélection ; ailleurs, la photo active.
    const ids = view === "grid" && selectedIds.length > 1 ? selectedIds : [selectedId];
    const targets = new Set(ids);
    const before = new Map(media.filter((m) => targets.has(m.id)).map((m) => [m.id, m] as const));
    if (before.size === 0) return;
    set((s) => ({
      media: s.media.map((m) => (targets.has(m.id) ? { ...m, ...patch } : m)),
    }));
    if (advance && ids.length === 1 && get().autoAdvance) get().move(1);
    try {
      await send(ids);
    } catch (e) {
      set((s) => ({
        media: s.media.map((m) => {
          const b = before.get(m.id);
          return b ? { ...m, rating: b.rating, flag: b.flag, color: b.color } : m;
        }),
        error: toCommandError(e),
      }));
    }
  }

  /**
   * Recharge albums, photos et corbeille après une opération sur des fichiers.
   * La photo active est gardée si elle est toujours là, sinon sa voisine.
   */
  async function refresh(): Promise<void> {
    const { libraryId, albumId, trash, media, selectedId } = get();
    if (libraryId === null) return;
    const index = Math.max(
      0,
      media.findIndex((m) => m.id === selectedId),
    );
    const [albums, list, summary] = await Promise.all([
      libraryApi.albumsList(libraryId),
      libraryApi.mediaList(libraryId, trash ? null : albumId, trash),
      libraryApi.trashSummary(libraryId),
    ]);
    const total = albums.reduce((n, a) => n + a.count, 0);
    set({
      albums,
      media: list,
      total,
      trashCount: summary.count,
      retentionDays: summary.retentionDays,
    });
    const keep = list.find((m) => m.id === selectedId) ?? list[Math.min(index, list.length - 1)];
    if (keep) await get().select(keep.id);
    else set({ selectedId: null, selectedIds: [], details: null, view: "grid" });
  }

  /** Signale un échec partiel : certains fichiers n'ont pas pu être déplacés. */
  function reportFailures(report: TrashReport): void {
    if (report.failed > 0) {
      set({ error: { code: "trashPartial", detail: `${report.failed}` } });
    }
  }

  return {
    libraryId: null,
    albums: [],
    albumId: null,
    trash: false,
    trashCount: 0,
    retentionDays: 30,
    notice: null,
    undoIds: null,
    media: [],
    total: 0,
    selectedId: null,
    selectedIds: [],
    details: null,
    versions: {},
    error: null,
    autoAdvance: readAutoAdvance(),
    view: "grid",
    zoom: null,
    navigatedAt: 0,

    load: async (libraryId) => {
      if (libraryId === null) {
        set({ libraryId, albums: [], media: [], total: 0, selectedId: null, details: null });
        return;
      }
      try {
        const same = get().libraryId === libraryId;
        const albumId = same ? get().albumId : null;
        const trash = same && get().trash;
        const [albums, media, summary] = await Promise.all([
          libraryApi.albumsList(libraryId),
          libraryApi.mediaList(libraryId, trash ? null : albumId, trash),
          libraryApi.trashSummary(libraryId),
        ]);
        const total = albums.reduce((n, a) => n + a.count, 0);
        const selectedId = media.some((m) => m.id === get().selectedId)
          ? get().selectedId
          : (media[0]?.id ?? null);
        set({
          libraryId,
          albums,
          albumId,
          trash,
          media,
          total,
          trashCount: summary.count,
          retentionDays: summary.retentionDays,
          error: null,
        });
        if (selectedId !== null) await get().select(selectedId);
        else set({ selectedId: null, details: null });
      } catch (e) {
        set({ error: toCommandError(e) });
      }
    },

    showAlbum: async (albumId) => {
      const { libraryId } = get();
      if (libraryId === null) return;
      try {
        const media = await libraryApi.mediaList(libraryId, albumId);
        set({ albumId, media, trash: false });
        const first = media[0];
        if (first) await get().select(first.id);
        else set({ selectedId: null, details: null });
      } catch (e) {
        set({ error: toCommandError(e) });
      }
    },

    select: async (mediaId, mode = "replace") => {
      const { media, selectedId, selectedIds } = get();
      let ids = [mediaId];
      let active = mediaId;
      if (mode === "toggle") {
        const removing = selectedIds.includes(mediaId) && selectedIds.length > 1;
        ids = removing ? selectedIds.filter((id) => id !== mediaId) : [...selectedIds, mediaId];
        // Photo retirée : la dernière restante devient active.
        active = removing ? (ids[ids.length - 1] ?? mediaId) : mediaId;
      } else if (mode === "range" && selectedId !== null) {
        const a = media.findIndex((m) => m.id === selectedId);
        const b = media.findIndex((m) => m.id === mediaId);
        if (a >= 0 && b >= 0) {
          ids = media.slice(Math.min(a, b), Math.max(a, b) + 1).map((m) => m.id);
        }
      }
      // Une autre photo s'affiche ajustée, comme dans Lightroom.
      set(mode === "replace" ? { selectedIds: ids, zoom: null } : { selectedIds: ids });
      await get().activate(active);
    },

    activate: async (mediaId) => {
      set((s) => ({
        selectedId: mediaId,
        selectedIds: s.selectedIds.includes(mediaId) ? s.selectedIds : [mediaId],
      }));
      try {
        const details = await libraryApi.mediaDetails(mediaId);
        // Une sélection plus récente a pu arriver pendant la requête.
        if (get().selectedId === mediaId) set({ details });
      } catch (e) {
        set({ error: toCommandError(e) });
      }
    },

    move: (delta, at) => {
      const { media, selectedId, view } = get();
      if (media.length === 0) return;
      if (view === "compare") {
        const ids = compareIds(get());
        const i = ids.indexOf(selectedId ?? -1);
        const next = ids[(i + delta + ids.length) % ids.length];
        if (next !== undefined) void get().activate(next);
        return;
      }
      const index = media.findIndex((m) => m.id === selectedId);
      const next = media[Math.min(Math.max(index + delta, 0), media.length - 1)];
      if (next && next.id !== selectedId) {
        set({ navigatedAt: at ?? performance.now() });
        void get().select(next.id);
      }
    },

    setView: (view) => {
      // La comparaison fige ses photos : la sélection en tient lieu.
      if (view === "compare") set({ selectedIds: compareIds(get()) });
      set({ view, zoom: null });
    },

    setZoom: (zoom) => set({ zoom }),

    previewReady: (mediaId) =>
      set((s) => ({
        versions: { ...s.versions, [mediaId]: (s.versions[mediaId] ?? 0) + 1 },
        media: s.media.map((m) => (m.id === mediaId ? { ...m, hasPreview: true } : m)),
      })),

    setRating: (rating) => mark({ rating }, (ids) => libraryApi.setRating(ids, rating), true),

    setFlag: (flag) => mark({ flag }, (ids) => libraryApi.setFlag(ids, flag), true),

    setAsCover: async (mediaId) => {
      const albumId = get().media.find((m) => m.id === mediaId)?.albumId;
      if (albumId === undefined) return;
      try {
        await libraryApi.setCover(albumId, mediaId);
        set((s) => ({
          albums: s.albums.map((a) =>
            a.id === albumId ? { ...a, coverMediaId: mediaId, coverChosen: true } : a,
          ),
        }));
      } catch (e) {
        set({ error: toCommandError(e) });
      }
    },

    showTrash: async () => {
      const { libraryId } = get();
      if (libraryId === null) return;
      try {
        const media = await libraryApi.mediaList(libraryId, null, true);
        set({ trash: true, media, view: "grid", zoom: null });
        const first = media[0];
        if (first) await get().select(first.id);
        else set({ selectedId: null, selectedIds: [], details: null });
      } catch (e) {
        set({ error: toCommandError(e) });
      }
    },

    targetIds: () => {
      const { selectedId, selectedIds, view } = get();
      if (selectedId === null) return [];
      return view === "grid" && selectedIds.length > 1 ? selectedIds : [selectedId];
    },

    rejectedIds: () =>
      get()
        .media.filter((m) => m.flag === -1)
        .map((m) => m.id),

    moveToTrash: async (ids = get().targetIds()) => {
      if (ids.length === 0 || get().trash) return;
      try {
        const report = await libraryApi.trash(ids);
        reportFailures(report);
        set({ notice: { kind: "trashed", count: report.done }, undoIds: ids });
        await refresh();
      } catch (e) {
        set({ error: toCommandError(e) });
      }
    },

    restore: async (ids = get().targetIds()) => {
      if (ids.length === 0) return;
      try {
        const report = await libraryApi.restore(ids);
        reportFailures(report);
        set({ notice: { kind: "restored", count: report.done }, undoIds: null });
        await refresh();
      } catch (e) {
        set({ error: toCommandError(e) });
      }
    },

    emptyTrash: async (ids) => {
      const { libraryId } = get();
      if (libraryId === null) return;
      try {
        const report = await libraryApi.emptyTrash(libraryId, ids ?? null);
        reportFailures(report);
        set({ notice: { kind: "purged", count: report.done }, undoIds: null });
        await refresh();
      } catch (e) {
        set({ error: toCommandError(e) });
      }
    },

    undoTrash: async () => {
      const ids = get().undoIds;
      if (ids) await get().restore(ids);
    },

    setRetentionDays: async (days) => {
      try {
        const retentionDays = await libraryApi.setRetention(days);
        set({ retentionDays });
      } catch (e) {
        set({ error: toCommandError(e) });
      }
    },

    toggleColor: (color) => {
      const current = get().media.find((m) => m.id === get().selectedId)?.color ?? null;
      const next = current === color ? null : color;
      return mark({ color: next }, (ids) => libraryApi.setColor(ids, next), false);
    },

    setAutoAdvance: (value) => {
      set({ autoAdvance: value });
      try {
        window.localStorage.setItem(AUTO_ADVANCE_KEY, String(value));
      } catch {
        // Préférence de confort : sans stockage, elle vaut pour la session.
      }
    },
  };
});
