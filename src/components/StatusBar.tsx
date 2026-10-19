import { useTranslation } from "react-i18next";
import { isFiltering, useLibrary } from "../store/library";

/** Barre d'état en bas de la fenêtre. */
export function StatusBar() {
  const { t } = useTranslation();
  const count = useLibrary((s) => s.media.length);
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
      <span title={t("status.safetyHint")}>{t("status.safety")}</span>
    </footer>
  );
}
