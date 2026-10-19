import { ChevronsRight, Columns2, Flag, FlagOff, LayoutGrid, Square, Star } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useLibrary } from "../store/library";
import { COLOR_LABELS } from "../ui/labels";
import { IconButton } from "./IconButton";

interface GridToolbarProps {
  count: number;
  /** Notes, drapeaux et couleurs : propres au tri. */
  culling?: boolean;
}

/** Grille (G), visionneuse (Entrée) ou comparaison (C). */
function ViewModes({ disabled }: { disabled: boolean }) {
  const { t } = useTranslation();
  const view = useLibrary((s) => s.view);
  const setView = useLibrary((s) => s.setView);
  const hasSelection = useLibrary((s) => s.selectedId !== null);
  const canCompare = useLibrary((s) => s.media.length >= 2 && s.selectedId !== null);
  return (
    <div className="toolbar-group">
      <IconButton
        icon={LayoutGrid}
        label={t("toolbar.grid")}
        shortcut="G"
        pressed={view === "grid"}
        disabled={disabled}
        onClick={() => setView("grid")}
      />
      <IconButton
        icon={Square}
        label={t("toolbar.loupe")}
        shortcut={t("keys.enter")}
        pressed={view === "loupe"}
        disabled={disabled || !hasSelection}
        onClick={() => setView("loupe")}
      />
      <IconButton
        icon={Columns2}
        label={t("toolbar.compare")}
        shortcut="C"
        pressed={view === "compare"}
        disabled={disabled || !canCompare}
        onClick={() => setView("compare")}
      />
    </div>
  );
}

/** Drapeaux, note et couleur de la photo sélectionnée, et avance automatique. */
function MarkTools({ disabled }: { disabled: boolean }) {
  const { t } = useTranslation();
  const selected = useLibrary((s) => s.media.find((m) => m.id === s.selectedId));
  const setRating = useLibrary((s) => s.setRating);
  const setFlag = useLibrary((s) => s.setFlag);
  const toggleColor = useLibrary((s) => s.toggleColor);
  const autoAdvance = useLibrary((s) => s.autoAdvance);
  const setAutoAdvance = useLibrary((s) => s.setAutoAdvance);
  const off = disabled || !selected;
  const rating = selected?.rating ?? 0;
  const flag = selected?.flag ?? 0;

  return (
    <>
      <span className="toolbar-divider" />
      <div className="toolbar-group">
        <IconButton
          icon={Flag}
          label={t("toolbar.pick")}
          shortcut="P"
          pressed={flag === 1}
          filled={flag === 1}
          disabled={off}
          onClick={() => void setFlag(flag === 1 ? 0 : 1)}
        />
        <IconButton
          icon={FlagOff}
          label={t("toolbar.reject")}
          shortcut="X"
          pressed={flag === -1}
          disabled={off}
          onClick={() => void setFlag(flag === -1 ? 0 : -1)}
        />
      </div>
      <div className="toolbar-group stars" role="group" aria-label={t("toolbar.rating")}>
        {[1, 2, 3, 4, 5].map((n) => (
          <IconButton
            key={n}
            icon={Star}
            label={t("toolbar.stars", { count: n })}
            shortcut={String(n)}
            filled={n <= rating}
            disabled={off}
            // Cliquer sur la note déjà posée la retire, comme dans Lightroom.
            onClick={() => void setRating(n === rating ? 0 : n)}
          />
        ))}
      </div>
      <div className="toolbar-group" role="group" aria-label={t("toolbar.colorLabel")}>
        {COLOR_LABELS.map((c, i) => (
          <button
            key={c}
            type="button"
            className={"color-chip color-" + c}
            aria-label={t("toolbar.colors." + c)}
            aria-pressed={selected?.color === c}
            title={
              i < 4 ? t("toolbar.colors." + c) + " (" + (i + 6) + ")" : t("toolbar.colors." + c)
            }
            disabled={off}
            onClick={() => void toggleColor(c)}
          />
        ))}
      </div>
      <span className="toolbar-divider" />
      <IconButton
        icon={ChevronsRight}
        label={t("toolbar.autoAdvance")}
        pressed={autoAdvance}
        onClick={() => setAutoAdvance(!autoAdvance)}
      />
    </>
  );
}

/** Barre d'outils sous l'espace de travail. */
export function GridToolbar({ count, culling }: GridToolbarProps) {
  const { t } = useTranslation();
  const empty = count === 0;

  return (
    <>
      <ViewModes disabled={empty} />
      {culling && <MarkTools disabled={empty} />}
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
