import { Star } from "lucide-react";
import { useTranslation } from "react-i18next";
import { compareIds, useLibrary } from "../store/library";
import { PhotoPane } from "./PhotoPane";

/**
 * Comparaison de 2 à 4 photos côte à côte. Le zoom et le déplacement sont
 * synchronisés : tous les panneaux montrent le même point, à 100 %. Le panneau
 * actif (cadre bleu) reçoit notes et drapeaux ; les flèches passent de l'un à l'autre.
 */
export function Compare() {
  const { t } = useTranslation();
  const media = useLibrary((s) => s.media);
  const selectedId = useLibrary((s) => s.selectedId);
  const selectedIds = useLibrary((s) => s.selectedIds);
  const versions = useLibrary((s) => s.versions);
  const zoom = useLibrary((s) => s.zoom);
  const setZoom = useLibrary((s) => s.setZoom);
  const activate = useLibrary((s) => s.activate);
  const ids = compareIds({ media, selectedId, selectedIds });
  const panes = ids.map((id) => media.find((m) => m.id === id)).filter((m) => m !== undefined);

  return (
    <div className="compare" data-count={panes.length}>
      {panes.map((m) => (
        <figure key={m.id} className="compare-pane" data-active={m.id === selectedId || undefined}>
          <PhotoPane
            media={m}
            version={versions[m.id] ?? 0}
            zoom={zoom}
            onZoom={setZoom}
            active={m.id === selectedId}
            onActivate={() => void activate(m.id)}
          />
          <figcaption className="compare-caption">
            <span className="compare-name">{m.fileName}</span>
            <span
              className="media-cell-stars"
              role="img"
              aria-label={t("toolbar.stars", { count: m.rating })}
            >
              {[1, 2, 3, 4, 5].map((n) => (
                <Star
                  key={n}
                  aria-hidden="true"
                  strokeWidth={1.5}
                  fill={n <= m.rating ? "currentColor" : "none"}
                  data-on={n <= m.rating || undefined}
                />
              ))}
            </span>
          </figcaption>
        </figure>
      ))}
    </div>
  );
}
