import { isTauri } from "@tauri-apps/api/core";
import { useRescan } from "./store/rescan";
import { type ExportFinished, type ExportProgress, useDeliver } from "./store/deliver";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useEffect } from "react";
import { events } from "./api/core";
import { type Device, useDevices } from "./store/devices";
import { useImport } from "./store/import";
import { useLayout } from "./store/layout";
import { useLibrary } from "./store/library";
import { startSecondarySync } from "./secondary/sync";
import { useNavigation } from "./store/navigation";
import { type BackgroundWarning, useWarnings } from "./store/warnings";
import {
  EVENT_OPEN_MAIN,
  EVENT_WELCOME_REFRESH,
  type MainAction,
  type WindowLabel,
  openMain,
  showSelf,
} from "./windows";

/** Applique dans la fenêtre principale une action venue de l'accueil. */
async function applyMainAction(action: MainAction): Promise<void> {
  const imp = useImport.getState();
  const library = useLibrary.getState();
  // L'accueil a pu créer une bibliothèque.
  await imp.loadLibraries();
  useNavigation.getState().goTo(action.screen);
  await showSelf();
  if (action.albumId !== undefined) await library.showAlbum(action.albumId);
  if (action.mediaId !== undefined) await library.select(action.mediaId);
  if (action.scanPath) {
    useDevices.getState().handled(action.scanPath);
    await imp.scanSource(action.scanPath);
  }
  if (action.resumeImportId !== undefined) await imp.resume(action.resumeImportId);
}

/** Dossier déposé sur la fenêtre : il devient la source de l'import. */
function onDrop(label: WindowLabel, path: string): void {
  if (useImport.getState().running) return;
  if (label === "welcome") {
    void openMain({ screen: "import", scanPath: path });
  } else {
    useNavigation.getState().goTo("import");
    void useImport.getState().scanSource(path);
  }
}

/**
 * Abonnements d'une fenêtre aux événements du cœur, montés une seule fois :
 * progression d'import, aperçus prêts, dépôt de dossier, échanges entre fenêtres.
 */
export function useAppEvents(label: WindowLabel): void {
  const libraryId = useImport((s) => s.libraryId);
  const running = useImport((s) => s.running);

  useEffect(() => {
    if (!isTauri()) return;
    void useImport.getState().loadLibraries();
    void useDevices.getState().load();
    const imp = useImport.getState();
    const subscriptions = [
      // Carte insérée ou retirée (relevé du cœur toutes les secondes).
      listen<Device>(
        "device-attached",
        (e) => void useDevices.getState().attached(e.payload, label),
      ),
      listen<Device>("device-detached", (e) => useDevices.getState().detached(e.payload)),
      // Disque d'une bibliothèque débranché ou rebranché : état et grille à jour.
      listen("library-status", () => {
        void useImport
          .getState()
          .loadLibraries()
          .then(() => useLibrary.getState().load(useImport.getState().libraryId));
      }),
      events.onImportProgress(imp.onProgress),
      listen<{ done: number; total: number }>("rescan-progress", (e) =>
        useRescan.getState().onProgress(e.payload),
      ),
      listen<ExportProgress>("export://progress", (e) =>
        useDeliver.getState().onProgress(e.payload),
      ),
      listen<ExportFinished>("export://finished", (e) =>
        useDeliver.getState().onFinished(e.payload),
      ),
      events.onImportFinished(imp.onFinished),
      listen<{ mediaId: number }>("preview-ready", (e) =>
        useLibrary.getState().previewReady(e.payload.mediaId),
      ),
      // XMP non écrits, aperçus impossibles : affichés dans la barre d'état.
      listen<BackgroundWarning>("background-warning", (e) =>
        useWarnings.getState().onWarning(e.payload),
      ),
      getCurrentWebview().onDragDropEvent((e) => {
        const layout = useLayout.getState();
        if (e.payload.type === "enter" || e.payload.type === "over") {
          if (!layout.dropping) layout.setDropping(true);
          return;
        }
        layout.setDropping(false);
        const path = e.payload.type === "drop" ? e.payload.paths[0] : undefined;
        if (path) onDrop(label, path);
      }),
      label === "main"
        ? listen<MainAction>(EVENT_OPEN_MAIN, (e) => void applyMainAction(e.payload))
        : listen(EVENT_WELCOME_REFRESH, () => void useImport.getState().loadLibraries()),
    ];
    // La fenêtre principale alimente le second écran.
    const stopSync = label === "main" ? startSecondarySync() : undefined;
    return () => {
      stopSync?.();
      for (const s of subscriptions) void s.then((unlisten) => unlisten());
    };
  }, [label]);

  // Recharger la bibliothèque quand elle change ou quand un import se termine.
  useEffect(() => {
    if (!isTauri() || running) return;
    void useLibrary.getState().load(libraryId);
  }, [libraryId, running]);
}
