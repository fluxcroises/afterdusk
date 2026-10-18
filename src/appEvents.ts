import { isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect } from "react";
import { events } from "./api/core";
import { useImport } from "./store/import";
import { useLibrary } from "./store/library";

/**
 * Abonnements de l'application aux événements du cœur, montés une seule fois :
 * progression d'import, aperçus prêts, bibliothèque courante.
 */
export function useAppEvents(): void {
  const libraryId = useImport((s) => s.libraryId);
  const running = useImport((s) => s.running);

  useEffect(() => {
    if (!isTauri()) return;
    void useImport.getState().loadLibraries();
    const imp = useImport.getState();
    const subscriptions = [
      events.onImportProgress(imp.onProgress),
      events.onImportFinished(imp.onFinished),
      listen<{ mediaId: number }>("preview-ready", (e) =>
        useLibrary.getState().previewReady(e.payload.mediaId),
      ),
    ];
    return () => {
      for (const s of subscriptions) void s.then((unlisten) => unlisten());
    };
  }, []);

  // Recharger la bibliothèque quand elle change ou quand un import se termine.
  useEffect(() => {
    if (!isTauri() || running) return;
    void useLibrary.getState().load(libraryId);
  }, [libraryId, running]);
}
