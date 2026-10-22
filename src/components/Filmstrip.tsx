import { mediaCount } from "../ui/counts";
import { Flag, Star } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useLibrary } from "../store/library";
import { COLOR_LABELS } from "../ui/labels";
import { FilmstripThumbs } from "./MediaGrid";

/** Pellicule : en-tête (source, nombre, filtre rapide) puis miniatures. */
export function Filmstrip() {
  const { t } = useTranslation();
  const media = useLibrary((s) => s.media);
  const albums = useLibrary((s) => s.albums);
  const albumId = useLibrary((s) => s.albumId);
  const selected = useLibrary((s) => s.media.find((m) => m.id === s.selectedId));
  const selectedCount = useLibrary((s) => s.selectedIds.length);
  const sourceName =
    albumId === null
      ? t("catalog.allPhotos")
      : (albums.find((a) => a.id === albumId)?.name ?? t("catalog.allPhotos"));

  return (
    <>
      <div className="filmstrip-header">
        <span className="filmstrip-source">{t("filmstrip.source", { source: sourceName })}</span>
        <span>
          {t("filmstrip.count", {
            count: selectedCount,
            items: mediaCount(
              t,
              media.filter((m) => m.kind !== "video").length,
              media.filter((m) => m.kind === "video").length,
            ),
          })}
          {selected && ` / ${selected.fileName}`}
        </span>
        <span className="toolbar-spacer" />
        <span className="filmstrip-filter" aria-label={t("filmstrip.filter")}>
          {t("filmstrip.filter")}
          <Flag aria-hidden="true" strokeWidth={1.75} />
          <span className="stars" aria-hidden="true">
            {[1, 2, 3, 4, 5].map((n) => (
              <Star key={n} strokeWidth={1.75} />
            ))}
          </span>
          <span className="color-labels" aria-hidden="true">
            {COLOR_LABELS.map((c) => (
              <span key={c} className={`color-chip color-${c}`} />
            ))}
          </span>
          <span className="filmstrip-filter-value">{t("filmstrip.noFilter")}</span>
        </span>
      </div>
      <div className="filmstrip-thumbs">
        {media.length > 0 ? (
          <FilmstripThumbs />
        ) : (
          <span className="filmstrip-empty">{t("filmstrip.empty")}</span>
        )}
      </div>
    </>
  );
}
