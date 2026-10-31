import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";
import { type CommandError, toCommandError } from "../api/core";
import { useLibrary, stackTop } from "./library";
import { useNavigation } from "./navigation";

/** État de la connexion au boîtier, miroir de `commands::tether::TetherStatus`. */
export interface TetherStatus {
  camera: string | null;
  active: boolean;
  connected: boolean;
  libraryId: number | null;
  albumName: string;
  folderId: number | null;
  received: number;
  remaining: number | null;
  leftOnCard: number;
  failed: string[];
}

/**
 * Durée d'affichage en grand de chaque photo reçue, en secondes ; 0 : jusqu'à
 * la suivante ; -1 : jamais.
 */
export type ShowFor = -1 | 0 | 3 | 5 | 10;
export const SHOW_FOR: ShowFor[] = [3, 5, 10, 0, -1];

/** Sous ce nombre de photos restantes, le bandeau prévient. */
export const LOW_REMAINING = 10;

const SHOW_FOR_KEY = "afterdusk.tetherShowFor";

const idle: TetherStatus = {
  camera: null,
  active: false,
  connected: false,
  libraryId: null,
  albumName: "",
  folderId: null,
  received: 0,
  remaining: null,
  leftOnCard: 0,
  failed: [],
};

function readShowFor(): ShowFor {
  try {
    const v = Number(window.localStorage.getItem(SHOW_FOR_KEY) ?? "5");
    return SHOW_FOR.includes(v as ShowFor) ? (v as ShowFor) : 5;
  } catch {
    return 5;
  }
}

/** « FUJIFILM X-H2 » → « X-H2 ». */
export function modelOf(camera: string): string {
  const space = camera.indexOf(" ");
  return space > 0 ? camera.slice(space + 1) : camera;
}

interface TetherState {
  status: TetherStatus;
  /** Dernier boîtier vu, pour nommer celui qui vient d'être débranché. */
  lastCamera: string | null;
  /** Boîtier dont le bandeau a été écarté. */
  dismissed: string | null;
  /** Avertissements déjà lus : photos restées sur la carte, photos illisibles. */
  seenLeft: number;
  seenFailed: number;
  starting: boolean;
  showFor: ShowFor;
  error: CommandError | null;
  load: () => Promise<void>;
  onStatus: (status: TetherStatus) => void;
  openStart: () => void;
  closeStart: () => void;
  start: (libraryId: number, albumName: string, backup: boolean) => Promise<void>;
  stop: () => Promise<void>;
  dismiss: () => void;
  acknowledge: () => void;
  setShowFor: (value: ShowFor) => void;
  /** Photos importées d'un dossier : celles de la séance s'affichent en grand. */
  imported: (payload: { folderId: number; libraryId: number; mediaIds: number[] }) => Promise<void>;
}

/** Affichage en grand en cours : fermeture prévue, sauf si l'utilisateur s'en sert. */
let closing: ReturnType<typeof setTimeout> | undefined;
let stopWatching: (() => void) | undefined;

function release(): void {
  clearTimeout(closing);
  closing = undefined;
  stopWatching?.();
  stopWatching = undefined;
}

/** Montre la photo en grand, puis revient à la grille après `seconds`. */
function show(mediaId: number, seconds: ShowFor): void {
  release();
  const library = useLibrary.getState();
  library.setZoom(null);
  library.setView("loupe");
  if (seconds === 0) return;
  // Souris bougée ou touche frappée : la photo reste ouverte.
  const keep = () => release();
  const arm = setTimeout(() => {
    window.addEventListener("pointermove", keep, { once: true });
    window.addEventListener("keydown", keep, { once: true });
  }, 300);
  stopWatching = () => {
    clearTimeout(arm);
    window.removeEventListener("pointermove", keep);
    window.removeEventListener("keydown", keep);
  };
  closing = setTimeout(() => {
    const now = useLibrary.getState();
    release();
    if (now.view === "loupe" && now.selectedId === mediaId) now.setView("grid");
  }, seconds * 1000);
}

export const useTether = create<TetherState>((set, get) => ({
  status: idle,
  lastCamera: null,
  dismissed: null,
  seenLeft: 0,
  seenFailed: 0,
  starting: false,
  showFor: readShowFor(),
  error: null,

  load: async () => {
    try {
      get().onStatus(await invoke<TetherStatus>("tether_status"));
    } catch {
      // Sans connexion au boîtier, le reste de l'application fonctionne.
    }
  },

  onStatus: (status) => {
    set((s) => ({
      status,
      lastCamera: status.camera ?? s.lastCamera,
      // Une nouvelle séance repart de zéro.
      ...(status.active && !s.status.active ? { seenLeft: 0, seenFailed: 0 } : {}),
    }));
    if (!status.active) release();
  },

  openStart: () => set({ starting: true, error: null }),
  closeStart: () => set({ starting: false, error: null }),

  start: async (libraryId, albumName, backup) => {
    try {
      await invoke("tether_start", {
        libraryId,
        albumName,
        settings: { backup, keywords: [], rights: null },
      });
      set({ starting: false, error: null });
      useNavigation.getState().goTo("cull");
      const library = useLibrary.getState();
      await library.reloadAlbums();
      const album = useLibrary.getState().albums.find((a) => a.name === albumName.trim());
      if (album) await library.showAlbum(album.id);
    } catch (e) {
      set({ error: toCommandError(e) });
    }
  },

  stop: async () => {
    release();
    try {
      await invoke("tether_stop");
    } catch (e) {
      set({ error: toCommandError(e) });
    }
  },

  dismiss: () => set((s) => ({ dismissed: s.status.camera })),

  acknowledge: () =>
    set((s) => ({ seenLeft: s.status.leftOnCard, seenFailed: s.status.failed.length })),

  setShowFor: (value) => {
    set({ showFor: value });
    try {
      window.localStorage.setItem(SHOW_FOR_KEY, String(value));
    } catch {
      // Préférence de confort : sans stockage, elle vaut pour la session.
    }
  },

  imported: async ({ folderId, libraryId, mediaIds }) => {
    const { status, showFor } = get();
    if (!status.active || status.folderId !== folderId || mediaIds.length === 0) return;
    const library = useLibrary.getState();
    if (library.libraryId !== libraryId) return;
    // Pas d'interruption hors de Trier et de la Bibliothèque (Retoucher, Livrer…).
    const screen = useNavigation.getState().screen;
    if (screen !== "cull" && screen !== "library") return;
    const album = library.albums.find((a) => a.name === status.albumName);
    if (album && library.albumId !== album.id) await library.showAlbum(album.id);
    const last = mediaIds[mediaIds.length - 1];
    const media = useLibrary.getState().media.find((m) => m.id === last);
    // RAW + JPEG : la pile s'affiche par son fichier du dessus.
    const id = media ? stackTop(media) : last;
    if (showFor === -1) return;
    await useLibrary.getState().select(id);
    show(id, showFor);
  },
}));
