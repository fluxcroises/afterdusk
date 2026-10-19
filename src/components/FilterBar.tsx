import { Flag, FlagOff, Search, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { type FlagValue, type RatingOp, isFiltering, useLibrary } from "../store/library";
import { useLayout } from "../store/layout";
import { COLOR_LABELS } from "../ui/labels";
import { IconButton } from "./IconButton";
import { FOCUS_SEARCH_EVENT, SEARCH_DELAY_MS } from "../search";

/** Champ de recherche : photos (cœur, index) ou albums (vue Albums). */
function SearchField() {
  const { t } = useTranslation();
  const showAlbums = useLibrary((s) => s.showAlbums);
  const query = useLibrary((s) => (s.showAlbums ? s.albumQuery : (s.filter.query ?? "")));
  const setFilter = useLibrary((s) => s.setFilter);
  const [draft, setDraft] = useState(query);
  const input = useRef<HTMLInputElement>(null);

  // La valeur change aussi de l'extérieur (Réinitialiser, vue Albums).
  const [shown, setShown] = useState(query);
  if (query !== shown) {
    setShown(query);
    setDraft(query);
  }

  useEffect(() => {
    if (draft === query) return;
    const timer = window.setTimeout(() => {
      if (showAlbums) useLibrary.setState({ albumQuery: draft });
      else void setFilter({ query: draft.trim() ? draft : undefined });
    }, SEARCH_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [draft, query, showAlbums, setFilter]);

  useEffect(() => {
    const focus = () => {
      input.current?.focus();
      input.current?.select();
    };
    window.addEventListener(FOCUS_SEARCH_EVENT, focus);
    return () => window.removeEventListener(FOCUS_SEARCH_EVENT, focus);
  }, []);

  return (
    <label className="filter-search" title={showAlbums ? undefined : t("filter.searchHint")}>
      <Search aria-hidden="true" strokeWidth={1.75} />
      <input
        ref={input}
        type="search"
        value={draft}
        placeholder={showAlbums ? t("filter.searchAlbums") : t("filter.search")}
        aria-label={t("filter.searchLabel")}
        onChange={(e) => setDraft(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            setDraft("");
            e.currentTarget.blur();
          }
        }}
      />
    </label>
  );
}

const RATING_CHOICES: { op: RatingOp; value: number }[] = [
  { op: "atLeast", value: 1 },
  { op: "atLeast", value: 2 },
  { op: "atLeast", value: 3 },
  { op: "atLeast", value: 4 },
  { op: "exactly", value: 5 },
  { op: "exactly", value: 0 },
];

/** Attributs et métadonnées des photos. */
function PhotoFilters() {
  const { t } = useTranslation();
  const filter = useLibrary((s) => s.filter);
  const options = useLibrary((s) => s.filterOptions);
  const setFilter = useLibrary((s) => s.setFilter);
  const flags = filter.flags ?? [];
  const colors = filter.colors ?? [];
  const toggleFlag = (flag: FlagValue) =>
    void setFilter({
      flags: flags.includes(flag) ? flags.filter((f) => f !== flag) : [...flags, flag],
    });
  const rating = filter.rating ? `${filter.rating.op}:${filter.rating.value}` : "";

  return (
    <>
      <span className="toolbar-divider" />
      <label className="toolbar-field">
        {t("filter.period")}
        <input
          type="date"
          aria-label={t("filter.from")}
          value={filter.from ?? ""}
          min={options?.firstDay ?? undefined}
          max={filter.to ?? options?.lastDay ?? undefined}
          onChange={(e) => void setFilter({ from: e.target.value || undefined })}
        />
        –
        <input
          type="date"
          aria-label={t("filter.to")}
          value={filter.to ?? ""}
          min={filter.from ?? options?.firstDay ?? undefined}
          max={options?.lastDay ?? undefined}
          onChange={(e) => void setFilter({ to: e.target.value || undefined })}
        />
      </label>
      <select
        aria-label={t("filter.rating")}
        value={rating}
        onChange={(e) => {
          const [op, value] = e.target.value.split(":");
          void setFilter({
            rating: op ? { op: op as RatingOp, value: Number(value) } : undefined,
          });
        }}
      >
        <option value="">{t("filter.anyRating")}</option>
        {RATING_CHOICES.map(({ op, value }) => (
          <option key={`${op}:${value}`} value={`${op}:${value}`}>
            {value === 0
              ? t("filter.unrated")
              : op === "atLeast"
                ? t("filter.atLeast", { stars: "★".repeat(value) })
                : "★".repeat(value)}
          </option>
        ))}
      </select>
      <div className="toolbar-group" role="group" aria-label={t("filter.flags")}>
        <IconButton
          icon={Flag}
          label={t("filter.picked")}
          filled
          pressed={flags.includes(1)}
          onClick={() => toggleFlag(1)}
        />
        <IconButton
          icon={Flag}
          label={t("filter.unflagged")}
          pressed={flags.includes(0)}
          onClick={() => toggleFlag(0)}
        />
        <IconButton
          icon={FlagOff}
          label={t("filter.rejected")}
          pressed={flags.includes(-1)}
          onClick={() => toggleFlag(-1)}
        />
      </div>
      <div className="toolbar-group" role="group" aria-label={t("filter.colors")}>
        {COLOR_LABELS.map((c) => (
          <button
            key={c}
            type="button"
            className={"color-chip color-" + c}
            aria-label={t("toolbar.colors." + c)}
            title={t("toolbar.colors." + c)}
            aria-pressed={colors.includes(c)}
            onClick={() =>
              void setFilter({
                colors: colors.includes(c) ? colors.filter((x) => x !== c) : [...colors, c],
              })
            }
          />
        ))}
      </div>
      <select
        aria-label={t("filter.camera")}
        value={filter.cameraId ?? ""}
        onChange={(e) =>
          void setFilter({ cameraId: e.target.value ? Number(e.target.value) : undefined })
        }
      >
        <option value="">{t("filter.anyCamera")}</option>
        {options?.cameras.map((g) => (
          <option key={g.id} value={g.id}>
            {g.name} ({g.count})
          </option>
        ))}
      </select>
      <select
        aria-label={t("filter.lens")}
        value={filter.lensId ?? ""}
        onChange={(e) =>
          void setFilter({ lensId: e.target.value ? Number(e.target.value) : undefined })
        }
      >
        <option value="">{t("filter.anyLens")}</option>
        {options?.lenses.map((g) => (
          <option key={g.id} value={g.id}>
            {g.name} ({g.count})
          </option>
        ))}
      </select>
    </>
  );
}

/** Barre de filtres au-dessus de la grille (\ pour la masquer, Ctrl+F pour chercher). */
export function FilterBar() {
  const { t } = useTranslation();
  const visible = useLayout((s) => s.filterBar);
  const showAlbums = useLibrary((s) => s.showAlbums);
  const filtering = useLibrary((s) => isFiltering(s.filter));
  const resetFilter = useLibrary((s) => s.resetFilter);
  if (!visible) return null;
  return (
    <div className="filter-bar" role="search">
      <SearchField />
      {!showAlbums && <PhotoFilters />}
      <span className="toolbar-spacer" />
      {filtering && !showAlbums && (
        <button
          type="button"
          className="button button-small filter-reset"
          onClick={() => void resetFilter()}
        >
          <X aria-hidden="true" strokeWidth={1.75} />
          {t("filter.reset")}
        </button>
      )}
    </div>
  );
}
