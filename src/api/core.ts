import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** Types miroirs des structures Rust (`src-tauri/src/commands`). */

export interface Library {
  id: number;
  name: string;
  rootPath: string;
  backupPath: string | null;
  folderTemplate: string;
  renameTemplate: string | null;
  isOnline: boolean;
}

export type MediaFormat = "raw" | "image";

export interface SourceEntry {
  path: string;
  fileName: string;
  size: number;
  format: MediaFormat;
  capturedAt: string;
  camera: string;
  alreadyImported: boolean;
}

export interface SourceDay {
  day: string;
  files: SourceEntry[];
}

export interface SourceView {
  root: string;
  days: SourceDay[];
  ignored: number;
}

export interface ImportOptions {
  libraryId: number;
  sourcePath: string;
  sourceLabel: string;
  files: string[];
  albumName: string;
  albumPerDay: boolean;
  rename: boolean;
  backup: boolean;
  keywords: string[];
  creator: string | null;
  rights: string | null;
}

export interface ImportProgress {
  importId: number;
  total: number;
  processed: number;
  imported: number;
  duplicates: number;
  failed: number;
  currentFile: string | null;
  mediaId: number | null;
}

export interface ResumableImport {
  id: number;
  libraryId: number;
  sourceLabel: string;
  sourcePath: string;
  startedAt: string;
  remaining: number;
  total: number;
}

/** Erreur renvoyée par le cœur : `code` est traduit via `errors.<code>`. */
export interface CommandError {
  code: string;
  detail: string;
}

export interface RecentImport {
  id: number;
  sourceLabel: string;
  date: string;
  count: number;
  mediaIds: number[];
}

export interface ImportFinished {
  importId: number;
  outcome: "done" | "interrupted" | null;
  error: CommandError | null;
}

export function isCommandError(value: unknown): value is CommandError {
  return typeof value === "object" && value !== null && "code" in value;
}

/** Toute erreur, ramenée à la forme d'une erreur du cœur. */
export function toCommandError(value: unknown): CommandError {
  return isCommandError(value) ? value : { code: "internal", detail: String(value) };
}

export const api = {
  librariesList: () => invoke<Library[]>("libraries_list"),
  libraryCreate: (name: string, root: string, backup: string | null) =>
    invoke<Library>("library_create", { name, root, backup }),
  sourceScan: (path: string, libraryId: number | null) =>
    invoke<SourceView>("source_scan", { path, libraryId }),
  importStart: (options: ImportOptions) => invoke<number>("import_start", { options }),
  importResume: (importId: number) => invoke<null>("import_resume", { importId }),
  importCancel: () => invoke<null>("import_cancel"),
  importsResumable: () => invoke<ResumableImport[]>("imports_resumable"),
  importsRecent: (libraryId: number) => invoke<RecentImport[]>("imports_recent", { libraryId }),
  appInfo: () => invoke<{ version: string; libraw: string }>("app_info"),
};

export const events = {
  onImportProgress: (handler: (p: ImportProgress) => void): Promise<UnlistenFn> =>
    listen<ImportProgress>("import-progress", (e) => handler(e.payload)),
  onImportFinished: (handler: (f: ImportFinished) => void): Promise<UnlistenFn> =>
    listen<ImportFinished>("import-finished", (e) => handler(e.payload)),
};
