import { isTauri } from "@tauri-apps/api/core";

interface ConfirmOptions {
  title: string;
  okLabel: string;
  cancelLabel: string;
}

/** Demande de confirmation native ; dans un navigateur (développement), celle du navigateur. */
export async function confirmWarning(text: string, options: ConfirmOptions): Promise<boolean> {
  if (!isTauri()) return window.confirm(text);
  const { ask } = await import("@tauri-apps/plugin-dialog");
  return ask(text, { ...options, kind: "warning" });
}
