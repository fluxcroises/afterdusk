import { Check } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { SourceEntry } from "../api/core";
import { sourceThumbnailUrl } from "../api/previews";
import { useImport } from "../store/import";
import { Thumbnail } from "./Thumbnail";

function dayLabel(day: string, language: string): string {
  const date = new Date(`${day}T12:00:00`);
  if (Number.isNaN(date.getTime())) return day;
  return new Intl.DateTimeFormat(language, {
    weekday: "long",
    day: "numeric",
    month: "long",
    year: "numeric",
  }).format(date);
}

function Tile({ file }: { file: SourceEntry }) {
  const { t } = useTranslation();
  const checked = useImport((s) => s.selected.has(file.path));
  const toggle = useImport((s) => s.toggle);
  const running = useImport((s) => s.running);
  const time = file.capturedAt.slice(11, 16);

  return (
    <button
      type="button"
      className="import-tile"
      role="checkbox"
      aria-checked={checked}
      aria-disabled={file.alreadyImported || undefined}
      data-imported={file.alreadyImported || undefined}
      disabled={file.alreadyImported || running}
      title={file.alreadyImported ? t("import.alreadyImported") : file.fileName}
      onClick={() => toggle(file.path)}
    >
      <span className="import-tile-check" aria-hidden="true">
        {checked && <Check strokeWidth={2.5} />}
      </span>
      <span className="import-tile-preview">
        <Thumbnail
          src={sourceThumbnailUrl(file.path)}
          fallback={file.fileName.split(".").pop()?.toUpperCase()}
        />
      </span>
      <span className="import-tile-name">{file.fileName}</span>
      <span className="import-tile-meta">
        {file.alreadyImported ? t("import.alreadyImported") : `${time} · ${file.camera || "—"}`}
      </span>
    </button>
  );
}

/** Médias de la source groupés par jour, déjà importés grisés. */
export function ImportGrid() {
  const { t, i18n } = useTranslation();
  const source = useImport((s) => s.source);
  const selected = useImport((s) => s.selected);
  const setDaySelected = useImport((s) => s.setDaySelected);
  const running = useImport((s) => s.running);
  if (!source) return null;

  return (
    <div className="import-grid">
      {source.days.map((day) => {
        const importable = day.files.filter((f) => !f.alreadyImported);
        const chosen = importable.filter((f) => selected.has(f.path)).length;
        const imported = day.files.length - importable.length;
        return (
          <section key={day.day} className="import-day">
            <header className="import-day-header">
              <label className="check-row">
                <input
                  type="checkbox"
                  checked={importable.length > 0 && chosen === importable.length}
                  ref={(el) => {
                    if (el) el.indeterminate = chosen > 0 && chosen < importable.length;
                  }}
                  disabled={importable.length === 0 || running}
                  onChange={(e) => setDaySelected(day.day, e.target.checked)}
                />
                <span className="import-day-title">{dayLabel(day.day, i18n.language)}</span>
              </label>
              <span className="import-day-count">
                {t("import.dayCount", { count: day.files.length })}
                {imported > 0 && ` · ${t("import.dayImported", { count: imported })}`}
              </span>
            </header>
            <div className="import-tiles">
              {day.files.map((file) => (
                <Tile key={file.path} file={file} />
              ))}
            </div>
          </section>
        );
      })}
    </div>
  );
}
