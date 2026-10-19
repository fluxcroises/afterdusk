import { create } from "zustand";
import type { AppInfo } from "../api/core";

/** Boîte « À propos d'Afterdusk », ouverte depuis le menu Aide (ou Afterdusk sur Mac). */
interface AboutState {
  open: boolean;
  show: () => void;
  close: () => void;
}

export const useAbout = create<AboutState>((set) => ({
  open: false,
  show: () => set({ open: true }),
  close: () => set({ open: false }),
}));

/** Texte copié par « Informations système… », à joindre à un signalement de bogue. */
export function systemReport(info: AppInfo | null, language: string): string {
  if (!info) return `Afterdusk (${navigator.userAgent})`;
  return [
    `Afterdusk ${info.version}`,
    `${info.os} ${info.arch}`,
    `WebView ${info.webview ?? "?"}`,
    `Tauri ${info.tauri} · LibRaw ${info.libraw} · SQLite ${info.sqlite}`,
    `Locale ${language}`,
  ].join("\n");
}
