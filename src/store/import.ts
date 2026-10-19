import { create } from "zustand";
import {
  api,
  type CommandError,
  type ImportFinished,
  type ImportProgress,
  type Library,
  type ResumableImport,
  type SourceView,
  toCommandError,
} from "../api/core";

export interface ImportForm {
  albumName: string;
  albumPerDay: boolean;
  rename: boolean;
  backup: boolean;
  keywords: string;
  rights: string;
}

interface ImportState {
  libraries: Library[];
  libraryId: number | null;
  source: SourceView | null;
  scanning: boolean;
  /** Chemins sélectionnés sur la source. */
  selected: Set<string>;
  form: ImportForm;
  progress: ImportProgress | null;
  running: boolean;
  lastResult: ImportFinished | null;
  resumable: ResumableImport[];
  error: CommandError | null;

  loadLibraries: () => Promise<void>;
  createLibrary: (root: string) => Promise<void>;
  selectLibrary: (id: number) => void;
  scanSource: (path: string) => Promise<void>;
  toggle: (path: string) => void;
  setDaySelected: (day: string, selected: boolean) => void;
  selectAll: (selected: boolean) => void;
  updateForm: (patch: Partial<ImportForm>) => void;
  start: (sourceLabel: string) => Promise<void>;
  resume: (importId: number) => Promise<void>;
  cancel: () => Promise<void>;
  onProgress: (progress: ImportProgress) => void;
  onFinished: (finished: ImportFinished) => void;
  dismissError: () => void;
}

const LIBRARY_KEY = "afterdusk.libraryId";

/** Bibliothèque courante, partagée par l'accueil et la fenêtre principale. */
function rememberLibrary(id: number): void {
  try {
    window.localStorage.setItem(LIBRARY_KEY, String(id));
  } catch {
    // Sans stockage, chaque fenêtre garde son propre choix pour la session.
  }
}

function rememberedLibrary(): number | null {
  try {
    const value = Number(window.localStorage.getItem(LIBRARY_KEY));
    return Number.isInteger(value) && value > 0 ? value : null;
  } catch {
    return null;
  }
}

const folderName = (path: string) => path.split(/[\\/]/).filter(Boolean).pop() ?? path;

/** Par défaut, tout ce qui n'a pas encore été importé est sélectionné. */
function newMedia(source: SourceView): Set<string> {
  return new Set(
    source.days.flatMap((d) => d.files.filter((f) => !f.alreadyImported).map((f) => f.path)),
  );
}

export const useImport = create<ImportState>((set, get) => ({
  libraries: [],
  libraryId: null,
  source: null,
  scanning: false,
  selected: new Set(),
  form: {
    albumName: "",
    albumPerDay: true,
    rename: false,
    backup: true,
    keywords: "",
    rights: "",
  },
  progress: null,
  running: false,
  lastResult: null,
  resumable: [],
  error: null,

  loadLibraries: async () => {
    try {
      const [libraries, resumable] = await Promise.all([
        api.librariesList(),
        api.importsResumable(),
      ]);
      // La bibliothèque choisie dans l'autre fenêtre fait foi, si elle existe encore.
      const remembered = rememberedLibrary();
      const valid = (id: number | null) => id !== null && libraries.some((l) => l.id === id);
      set((s) => ({
        libraries,
        resumable,
        libraryId: valid(remembered)
          ? remembered
          : valid(s.libraryId)
            ? s.libraryId
            : (libraries[0]?.id ?? null),
      }));
    } catch (e) {
      set({ error: toCommandError(e) });
    }
  },

  createLibrary: async (root) => {
    try {
      const library = await api.libraryCreate(folderName(root), root, null);
      rememberLibrary(library.id);
      set((s) => ({ libraries: [...s.libraries, library], libraryId: library.id }));
      const source = get().source;
      if (source) await get().scanSource(source.root);
    } catch (e) {
      set({ error: toCommandError(e) });
    }
  },

  selectLibrary: (id) => {
    rememberLibrary(id);
    set({ libraryId: id });
    const source = get().source;
    if (source) void get().scanSource(source.root);
  },

  scanSource: async (path) => {
    set({ scanning: true, error: null });
    try {
      const source = await api.sourceScan(path, get().libraryId);
      set({ source, selected: newMedia(source), scanning: false });
    } catch (e) {
      set({ scanning: false, error: toCommandError(e) });
    }
  },

  toggle: (path) =>
    set((s) => {
      const selected = new Set(s.selected);
      if (selected.has(path)) selected.delete(path);
      else selected.add(path);
      return { selected };
    }),

  setDaySelected: (day, value) =>
    set((s) => {
      const selected = new Set(s.selected);
      const files = s.source?.days.find((d) => d.day === day)?.files ?? [];
      for (const f of files) {
        if (f.alreadyImported) continue;
        if (value) selected.add(f.path);
        else selected.delete(f.path);
      }
      return { selected };
    }),

  selectAll: (value) =>
    set((s) => ({ selected: value && s.source ? newMedia(s.source) : new Set() })),

  updateForm: (patch) => set((s) => ({ form: { ...s.form, ...patch } })),

  start: async (sourceLabel) => {
    const { libraryId, source, selected, form, libraries } = get();
    if (libraryId === null || !source || selected.size === 0) return;
    const library = libraries.find((l) => l.id === libraryId);
    set({ running: true, progress: null, lastResult: null, error: null });
    try {
      await api.importStart({
        libraryId,
        sourcePath: source.root,
        sourceLabel,
        files: [...selected],
        albumName: form.albumName.trim(),
        albumPerDay: form.albumPerDay,
        rename: form.rename,
        backup: form.backup && !!library?.backupPath,
        keywords: form.keywords
          .split(",")
          .map((k) => k.trim())
          .filter(Boolean),
        creator: null,
        rights: form.rights.trim() || null,
      });
    } catch (e) {
      set({ running: false, error: toCommandError(e) });
    }
  },

  resume: async (importId) => {
    set({ running: true, progress: null, lastResult: null, error: null });
    try {
      await api.importResume(importId);
    } catch (e) {
      set({ running: false, error: toCommandError(e) });
    }
  },

  cancel: async () => {
    try {
      await api.importCancel();
    } catch (e) {
      set({ error: toCommandError(e) });
    }
  },

  onProgress: (progress) => set({ progress, running: true }),

  onFinished: (finished) => {
    set({ running: false, lastResult: finished, error: finished.error });
    // Les photos importées apparaissent désormais grisées.
    const source = get().source;
    void get().loadLibraries();
    if (source) void get().scanSource(source.root);
  },

  dismissError: () => set({ error: null }),
}));
