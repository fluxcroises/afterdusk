import { useTranslation } from "react-i18next";
import { isFiltering, useLibrary } from "../store/library";
import { useRescan } from "../store/rescan";

/** Barre d'état en bas de la fenêtre. */
export function StatusBar() {
  const { t } = useTranslation();
  // Une pile dépliée compte pour une photo.
  const count = useLibrary(
    (s) =>
      s.media.filter((m) => m.companionOf === null || !s.media.some((x) => x.id === m.companionOf))
        .length,
  );
  const selected = useLibrary((s) => s.selectedIds.length);
  // Photos de la source affichée, avant filtre.
  const total = useLibrary((s) =>
    s.trash
      ? s.trashCount
      : s.albumId === null
        ? s.total
        : (s.albums.find((a) => a.id === s.albumId)?.count ?? 0),
  );
  const filtering = useLibrary((s) => isFiltering(s.filter));
  const albums = useLibrary((s) => (s.showAlbums ? s.albums.length : null));
  // Valeurs simples : un objet recréé à chaque lecture relancerait le rendu sans fin.
  const rescanning = useRescan((s) => s.running);
  const rescanDone = useRescan((s) => s.progress?.done ?? 0);
  const rescanTotal = useRescan((s) => s.progress?.total ?? 0);
  return (
    <footer className="status-bar">
      <span>
        {albums !== null
          ? t("status.albums", { count: albums })
          : filtering
            ? t("status.filtered", { count, total, selected })
            : t("status.selection", { count, selected })}
      </span>
      <span className="status-bar-spacer" />
      {rescanning && <span>{t("rescan.progress", { done: rescanDone, total: rescanTotal })}</span>}
      <span title={t("status.safetyHint")}>{t("status.safety")}</span>
    </footer>
  );
}
