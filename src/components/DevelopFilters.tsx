import { Check, Trash2, WandSparkles } from "lucide-react";
import { type KeyboardEvent, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { type Film, type PresetSummary, type PresetThumb, useDevelop } from "../store/develop";
import { confirmWarning } from "../ui/confirm";
import { Slider } from "./DevelopPanels";
import { IconButton } from "./IconButton";
import { PanelSection } from "./PanelSection";
import { PRESET_DEFAULT_KEYS, PRESET_SECTIONS, pathsOf } from "../ui/sections";
import { SectionPicker } from "./SettingsSections";

/** Luminance relative d'une couleur « #rrggbb », pour lire le texte posé dessus. */
function isLight(hex: string): boolean {
  const n = parseInt(hex.slice(1), 16);
  const [r, g, b] = [(n >> 16) & 255, (n >> 8) & 255, n & 255];
  return 0.2126 * r + 0.7152 * g + 0.0722 * b > 150;
}

/**
 * Boîte de pellicule dessinée pour Afterdusk (aucun nom ni logo de marque) :
 * couleurs évoquant le rendu, sensibilité en gros chiffres, bande du type de
 * film.
 */
export function FilmBox({ film }: { film: Film }) {
  const { t } = useTranslation();
  const [main, band] = film.colors;
  const ink = isLight(main) ? "#1a1a1a" : "#ffffff";
  const bandInk = isLight(band) ? "#1a1a1a" : "#ffffff";
  const label = t(`develop.presets.filmShort.${film.kind}`);
  return (
    <svg className="film-box" viewBox="0 0 48 64" aria-hidden="true">
      <rect x="1" y="1" width="46" height="62" rx="3" fill={main} stroke="rgb(0 0 0 / 0.35)" />
      <rect x="1" y="40" width="46" height="12" fill={band} />
      <rect x="1" y="1" width="46" height="8" rx="3" fill="rgb(255 255 255 / 0.18)" />
      <text
        x="24"
        y="31"
        textAnchor="middle"
        fontSize={film.iso >= 1000 ? 13 : 17}
        fontWeight="800"
        fill={ink}
      >
        {film.iso}
      </text>
      <text x="24" y="49.5" textAnchor="middle" fontSize="7.5" fontWeight="700" fill={bandInk}>
        {label}
      </text>
    </svg>
  );
}

/** Vignette dessinée dans un canevas, ajustée au carré de la tuile. */
function ThumbCanvas({ thumb }: { thumb: PresetThumb | undefined }) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const canvas = ref.current;
    const context = canvas?.getContext("2d");
    if (!canvas || !context || !thumb) return;
    canvas.width = thumb.width;
    canvas.height = thumb.height;
    context.putImageData(new ImageData(thumb.pixels, thumb.width, thumb.height), 0, 0);
  }, [thumb]);
  return <canvas ref={ref} className="filter-thumb" data-loading={thumb ? undefined : true} />;
}

interface TileProps {
  label: string;
  thumb: PresetThumb | undefined;
  checked: boolean;
  film?: Film | null;
  hint?: string;
  onClick: () => void;
  onRename?: (to: string) => Promise<boolean>;
  onDelete?: () => void;
}

/** Tuile de la galerie : la photo ouverte avec ce preset, son nom, la coche. */
function FilterTile({ label, thumb, checked, film, hint, onClick, onRename, onDelete }: TileProps) {
  const { t } = useTranslation();
  const [editing, setEditing] = useState<string | null>(null);
  const commit = async () => {
    if (editing === null || !onRename) return;
    const to = editing.trim();
    if (to === "" || to === label || (await onRename(to))) setEditing(null);
  };
  const onKey = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") void commit();
    if (e.key === "Escape") setEditing(null);
  };
  return (
    <div className="filter-tile" data-checked={checked || undefined}>
      <button
        type="button"
        className="filter-tile-button"
        aria-pressed={checked}
        title={hint ?? label}
        onClick={onClick}
      >
        <span className="filter-tile-image">
          <ThumbCanvas thumb={thumb} />
          {film && <FilmBox film={film} />}
          {checked && (
            <span className="filter-tile-check">
              <Check aria-hidden="true" strokeWidth={3} />
            </span>
          )}
        </span>
      </button>
      {editing !== null ? (
        <input
          className="develop-name-input filter-tile-input"
          value={editing}
          maxLength={80}
          autoFocus
          aria-label={label}
          onChange={(e) => setEditing(e.currentTarget.value)}
          onKeyDown={onKey}
          onBlur={() => void commit()}
        />
      ) : (
        <span
          className="filter-tile-label"
          title={onRename ? t("develop.presets.renameHint") : undefined}
          onDoubleClick={onRename ? () => setEditing(label) : undefined}
        >
          {label}
        </span>
      )}
      {onDelete && (
        <span className="filter-tile-delete">
          <IconButton icon={Trash2} label={t("develop.presets.delete")} onClick={onDelete} />
        </span>
      )}
    </div>
  );
}

/** Formulaire du nouveau preset : nom et réglages gardés. */
function CreatePreset() {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [keys, setKeys] = useState<string[]>(PRESET_DEFAULT_KEYS);
  const disabled = useDevelop((s) => s.settings === null);
  const createPreset = useDevelop((s) => s.createPreset);

  if (!open) {
    return (
      <button
        type="button"
        className="button button-small develop-preset-create"
        disabled={disabled}
        onClick={() => setOpen(true)}
      >
        {t("develop.presets.create")}
      </button>
    );
  }
  const save = async () => {
    if (name.trim() === "" || keys.length === 0) return;
    if (await createPreset(name.trim(), pathsOf(PRESET_SECTIONS, keys))) {
      setOpen(false);
      setName("");
    }
  };
  return (
    <form
      className="develop-preset-form"
      onSubmit={(e) => {
        e.preventDefault();
        void save();
      }}
    >
      <label className="develop-preset-name">
        <span>{t("develop.presets.name")}</span>
        <input
          type="text"
          value={name}
          maxLength={80}
          autoFocus
          onChange={(e) => setName(e.currentTarget.value)}
        />
      </label>
      <SectionPicker options={PRESET_SECTIONS} checked={keys} onChange={setKeys} />
      <div className="develop-crop-actions">
        <span className="toolbar-spacer" />
        <button type="button" className="button button-small" onClick={() => setOpen(false)}>
          {t("develop.crop.cancel")}
        </button>
        <button
          type="submit"
          className="button button-small button-primary"
          disabled={name.trim() === "" || keys.length === 0}
        >
          {t("develop.presets.save")}
        </button>
      </div>
    </form>
  );
}

/**
 * Galerie de filtres (cahier des charges, 4.5) : « Optimisation
 * automatique » en tête, vignette « Original », puis les presets en grille
 * de trois colonnes par groupes repliables, chaque vignette montrant la photo
 * ouverte ainsi réglée ; intensité du preset choisi.
 */
export function FiltersPanel() {
  const { t } = useTranslation();
  const groups = useDevelop((s) => s.presetGroups);
  const thumbs = useDevelop((s) => s.presetThumbs);
  const look = useDevelop((s) => s.settings?.look);
  const busy = useDevelop((s) => s.autoBusy);
  const disabled = useDevelop((s) => s.settings === null);
  const st = useDevelop.getState();

  const nameOf = (p: PresetSummary) => p.name ?? t(`develop.presets.items.${p.id}`);
  const filmHint = (p: PresetSummary) =>
    p.film
      ? `${nameOf(p)} · ${t(`develop.presets.films.${p.film.kind}`)} · ${p.film.iso} ISO`
      : undefined;
  const remove = async (p: PresetSummary) => {
    const ok = await confirmWarning(t("develop.presets.confirmDelete", { name: nameOf(p) }), {
      title: t("develop.presets.title"),
      okLabel: t("develop.presets.delete"),
      cancelLabel: t("trash.cancel"),
    });
    if (ok) await st.deletePreset(p.id);
  };

  return (
    <>
      <PanelSection id="develop.filters" title={t("develop.presets.title")}>
        <button
          type="button"
          className="button develop-optimize"
          disabled={disabled || busy}
          title={t("develop.autoHint")}
          onClick={() => void st.autoLight()}
        >
          <WandSparkles aria-hidden="true" strokeWidth={1.75} />
          {t("develop.presets.autoOptimize")}
        </button>
        <div className="filter-grid">
          <FilterTile
            label={t("develop.presets.original")}
            thumb={thumbs[""]}
            checked={!look}
            onClick={() => {
              if (look) void st.applyPreset(null);
            }}
          />
        </div>
        {look && (
          <Slider
            label={t("develop.presets.amount")}
            display={t("develop.lens.percent", { value: look.amount })}
            value={look.amount}
            min={0}
            max={100}
            step={1}
            onChange={(v) => void st.setPresetAmount(v)}
            onReset={() => void st.setPresetAmount(100)}
          />
        )}
      </PanelSection>
      {groups?.map((group) => (
        <PanelSection
          key={group.id}
          id={`develop.filters.${group.id}`}
          title={t(`develop.presets.groups.${group.id}`)}
          defaultCollapsed={!["essentials", "film"].includes(group.id)}
        >
          {group.user && group.presets.length === 0 && (
            <p className="panel-note">{t("develop.presets.userEmpty")}</p>
          )}
          {group.presets.length > 0 && (
            <div className="filter-grid">
              {group.presets.map((p) => (
                <FilterTile
                  key={p.id}
                  label={nameOf(p)}
                  thumb={thumbs[p.id]}
                  checked={look?.id === p.id}
                  film={p.film}
                  hint={filmHint(p)}
                  onClick={() => {
                    if (look?.id !== p.id) void st.applyPreset(p.id);
                  }}
                  onRename={group.user ? (to) => st.renamePreset(p.id, to) : undefined}
                  onDelete={group.user ? () => void remove(p) : undefined}
                />
              ))}
            </div>
          )}
          {group.user && <CreatePreset />}
        </PanelSection>
      ))}
    </>
  );
}
