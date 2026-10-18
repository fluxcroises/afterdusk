import { Flag, Star } from "lucide-react";
import { useTranslation } from "react-i18next";
import { COLOR_LABELS } from "../ui/labels";

/** Pellicule : en-tête (source, nombre, filtre rapide) puis miniatures. */
export function Filmstrip() {
  const { t } = useTranslation();
  return (
    <>
      <div className="filmstrip-header">
        <span className="filmstrip-source">
          {t("filmstrip.source", { source: t("catalog.allPhotos") })}
        </span>
        <span>{t("filmstrip.count", { count: 0, selected: 0 })}</span>
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
        <span className="filmstrip-empty">{t("filmstrip.empty")}</span>
      </div>
    </>
  );
}
