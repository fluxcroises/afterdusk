import { Columns2, Flag, FlagOff, LayoutGrid, Square, Star } from "lucide-react";
import { useTranslation } from "react-i18next";
import { COLOR_LABELS } from "../ui/labels";
import { IconButton } from "./IconButton";

interface GridToolbarProps {
  count: number;
  /** Notes, drapeaux et couleurs : propres au tri. */
  culling?: boolean;
}

/** Barre d'outils sous l'espace de travail. */
export function GridToolbar({ count, culling }: GridToolbarProps) {
  const { t } = useTranslation();
  const empty = count === 0;

  return (
    <>
      <div className="toolbar-group">
        <IconButton
          icon={LayoutGrid}
          label={t("toolbar.grid")}
          shortcut="G"
          pressed
          disabled={empty}
        />
        <IconButton icon={Square} label={t("toolbar.loupe")} shortcut="E" disabled={empty} />
        <IconButton icon={Columns2} label={t("toolbar.compare")} shortcut="C" disabled={empty} />
      </div>
      {culling && (
        <>
          <span className="toolbar-divider" />
          <div className="toolbar-group">
            <IconButton icon={Flag} label={t("toolbar.pick")} shortcut="P" disabled={empty} />
            <IconButton icon={FlagOff} label={t("toolbar.reject")} shortcut="X" disabled={empty} />
          </div>
          <div className="toolbar-group stars" role="group" aria-label={t("toolbar.rating")}>
            {[1, 2, 3, 4, 5].map((n) => (
              <IconButton
                key={n}
                icon={Star}
                label={t("toolbar.stars", { count: n })}
                shortcut={String(n)}
                disabled={empty}
              />
            ))}
          </div>
          <div className="toolbar-group" role="group" aria-label={t("toolbar.colorLabel")}>
            {COLOR_LABELS.map((c, i) => (
              <button
                key={c}
                type="button"
                className={`color-chip color-${c}`}
                aria-label={t(`toolbar.colors.${c}`)}
                title={i < 4 ? `${t(`toolbar.colors.${c}`)} (${i + 6})` : t(`toolbar.colors.${c}`)}
                disabled={empty}
              />
            ))}
          </div>
        </>
      )}
      <span className="toolbar-spacer" />
      <label className="toolbar-field">
        {t("toolbar.sort")}
        <select disabled={empty} defaultValue="captureTime">
          <option value="captureTime">{t("toolbar.sortCaptureTime")}</option>
          <option value="fileName">{t("toolbar.sortFileName")}</option>
          <option value="rating">{t("toolbar.sortRating")}</option>
        </select>
      </label>
    </>
  );
}
