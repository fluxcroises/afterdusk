import { Info, Trash2 } from "lucide-react";
import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { useLibrary } from "../store/library";
import { emptyTrash } from "../trash";

/** Durée d'affichage de l'annonce, le temps de cliquer sur Annuler. */
export const NOTICE_MS = 8000;

/** Annonce de la dernière opération sur la corbeille ; un envoi s'annule ici ou par Ctrl+Z, même après. */
export function TrashNotice() {
  const { t } = useTranslation();
  const notice = useLibrary((s) => s.notice);
  const undoTrash = useLibrary((s) => s.undoTrash);

  useEffect(() => {
    if (!notice) return;
    const timer = window.setTimeout(() => {
      if (useLibrary.getState().notice === notice) useLibrary.setState({ notice: null });
    }, NOTICE_MS);
    return () => window.clearTimeout(timer);
  }, [notice]);

  if (!notice) return null;
  return (
    <div className="banner banner-info" role="status">
      <Trash2 aria-hidden="true" strokeWidth={1.75} />
      <span>{t(`trash.notice.${notice.kind}`, { count: notice.count })}</span>
      {notice.kind === "trashed" && (
        <button type="button" className="button button-small" onClick={() => void undoTrash()}>
          {t("trash.undo")}
        </button>
      )}
    </div>
  );
}

/** En tête de la vue Corbeille : délai de suppression, restaurer, vider. */
export function TrashBar() {
  const { t } = useTranslation();
  const trash = useLibrary((s) => s.trash);
  const count = useLibrary((s) => s.trashCount);
  const days = useLibrary((s) => s.retentionDays);
  const hasSelection = useLibrary((s) => s.selectedId !== null);
  const restore = useLibrary((s) => s.restore);
  if (!trash) return null;
  return (
    <div className="banner banner-info">
      <Info aria-hidden="true" strokeWidth={1.75} />
      <span>{t("trash.retentionNote", { count: days })}</span>
      <button
        type="button"
        className="button button-small"
        disabled={!hasSelection}
        onClick={() => void restore()}
      >
        {t("menu.restore")}
      </button>
      <button
        type="button"
        className="button button-small"
        disabled={count === 0}
        onClick={() => void emptyTrash()}
      >
        {t("trash.emptyEllipsis")}
      </button>
    </div>
  );
}
