import i18n from "./i18n";
import { useLibrary } from "./store/library";
import { confirmWarning as confirmDialog } from "./ui/confirm";

/** Demande de confirmation des opérations de corbeille. */
function confirmWarning(text: string, okLabel: string): Promise<boolean> {
  return confirmDialog(text, {
    title: i18n.t("trash.confirmTitle"),
    okLabel,
    cancelLabel: i18n.t("trash.cancel"),
  });
}

/** Photo > Envoyer les rejetées dans la corbeille : sur confirmation, réversible. */
export async function trashRejected(): Promise<void> {
  const library = useLibrary.getState();
  const ids = library.rejectedIds();
  if (ids.length === 0) {
    useLibrary.setState({ error: { code: "noRejected", detail: "" } });
    return;
  }
  const ok = await confirmWarning(
    i18n.t("trash.confirmRejected", { count: ids.length }),
    i18n.t("trash.moveRejected"),
  );
  if (ok) await library.moveToTrash(ids);
}

/** Vide toute la corbeille de la bibliothèque, après confirmation. */
export async function emptyTrash(): Promise<void> {
  const library = useLibrary.getState();
  if (library.trashCount === 0) return;
  const ok = await confirmWarning(
    i18n.t("trash.confirmEmpty", { count: library.trashCount }),
    i18n.t("trash.empty"),
  );
  if (ok) await library.emptyTrash();
}

/** Supprime définitivement les photos choisies dans la corbeille, après confirmation. */
export async function purgeSelection(): Promise<void> {
  const library = useLibrary.getState();
  const ids = library.targetIds();
  if (!library.trash || ids.length === 0) return;
  const ok = await confirmWarning(
    i18n.t("trash.confirmPurge", { count: ids.length }),
    i18n.t("trash.purge"),
  );
  if (ok) await library.emptyTrash(ids);
}
