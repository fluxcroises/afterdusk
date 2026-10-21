import { open as chooseFiles } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { type LocateSource, useLocate } from "../store/locate";
import { formatOffset } from "../ui/format";
import { ErrorBanner } from "./ErrorBanner";

/** Décalage de l'ordinateur à cette date, pour les boîtiers sans fuseau. */
function computerOffset(): number {
  return -new Date().getTimezoneOffset();
}

const DEFAULT_MINUTES: Record<LocateSource, number> = { gpx: 10, phone: 30 };

/** Photo > Localiser… : trace GPX ou photos de téléphone. */
export function LocateDialog() {
  const ids = useLocate((s) => s.ids);
  return ids ? <LocateForm count={ids.length} /> : null;
}

function LocateForm({ count }: { count: number }) {
  const { t } = useTranslation();
  const clock = useLocate((s) => s.clock);
  const running = useLocate((s) => s.running);
  const report = useLocate((s) => s.report);
  const error = useLocate((s) => s.error);
  const [source, setSource] = useState<LocateSource>("gpx");
  const [files, setFiles] = useState<string[]>([]);
  const [useWritten, setUseWritten] = useState(true);
  const [offset, setOffset] = useState<number | null>(null);
  const [minutes, setMinutes] = useState(DEFAULT_MINUTES.gpx);
  const [replace, setReplace] = useState(false);
  const written = clock?.writtenMinutes ?? null;
  const manual = offset ?? written ?? computerOffset();
  const canUseWritten = written !== null;

  useEffect(() => {
    const escape = (e: KeyboardEvent) => {
      if (e.key === "Escape") useLocate.getState().close();
    };
    window.addEventListener("keydown", escape);
    return () => window.removeEventListener("keydown", escape);
  }, []);

  const pick = async () => {
    const chosen = await chooseFiles({
      multiple: true,
      title: t(`locate.choose.${source}`),
      filters:
        source === "gpx"
          ? [{ name: "GPX", extensions: ["gpx"] }]
          : [{ name: t("locate.phonePhotos"), extensions: ["jpg", "jpeg", "heic", "heif", "dng"] }],
    });
    if (Array.isArray(chosen)) setFiles(chosen);
    else if (typeof chosen === "string") setFiles([chosen]);
  };

  const close = () => useLocate.getState().close();
  const changeSource = (s: LocateSource) => {
    setSource(s);
    setFiles([]);
    setMinutes(DEFAULT_MINUTES[s]);
  };

  return (
    <div className="modal-backdrop" onMouseDown={(e) => e.target === e.currentTarget && close()}>
      <form
        className="copy-dialog locate-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="locate-title"
        onSubmit={(e) => {
          e.preventDefault();
          void useLocate.getState().run({
            source,
            files,
            offsetMinutes: useWritten && canUseWritten ? null : manual,
            minutes,
            replace,
          });
        }}
      >
        <h2 id="locate-title" className="copy-title">
          {t("locate.title", { count })}
        </h2>
        <ErrorBanner error={error} onDismiss={() => useLocate.setState({ error: null })} />
        <div className="develop-segmented" role="radiogroup" aria-label={t("locate.source")}>
          {(["gpx", "phone"] as const).map((s) => (
            <button
              key={s}
              type="button"
              role="radio"
              aria-checked={source === s}
              onClick={() => changeSource(s)}
            >
              {t(`locate.sources.${s}`)}
            </button>
          ))}
        </div>
        <div className="locate-files">
          <button type="button" className="button button-small" onClick={() => void pick()}>
            {t(`locate.choose.${source}`)}
          </button>
          <span className="panel-note">
            {files.length === 0
              ? t("locate.noFile")
              : files.length === 1
                ? (files[0].split(/[\\/]/).pop() ?? files[0])
                : t("locate.files", { count: files.length })}
          </span>
        </div>
        <fieldset className="form-fieldset locate-clock">
          <legend className="field-row-label">{t("locate.clock")}</legend>
          <label className="check-row">
            <input
              type="radio"
              name="locate-clock"
              checked={useWritten && canUseWritten}
              disabled={!canUseWritten}
              onChange={() => setUseWritten(true)}
            />
            {written !== null
              ? t("locate.written", {
                  offset: formatOffset(written),
                  count: clock?.withOffset ?? 0,
                  total: clock?.total ?? 0,
                })
              : t("locate.writtenNone")}
          </label>
          <label className="check-row">
            <input
              type="radio"
              name="locate-clock"
              checked={!useWritten || !canUseWritten}
              onChange={() => setUseWritten(false)}
            />
            {t("locate.utc")}
            <input
              type="number"
              className="locate-offset"
              aria-label={t("locate.offsetHours")}
              step={0.25}
              min={-14}
              max={14}
              value={manual / 60}
              onFocus={() => setUseWritten(false)}
              onChange={(e) => {
                setUseWritten(false);
                setOffset(Math.round(Number(e.currentTarget.value) * 60));
              }}
            />
            {t("locate.hours")}
          </label>
        </fieldset>
        <label className="deliver-field">
          <span>{t(`locate.minutes.${source}`)}</span>
          <input
            type="number"
            min={1}
            max={240}
            value={minutes}
            onChange={(e) => setMinutes(Math.max(1, Math.round(Number(e.currentTarget.value))))}
          />
        </label>
        <label className="check-row">
          <input
            type="checkbox"
            checked={replace}
            onChange={(e) => setReplace(e.currentTarget.checked)}
          />
          {t("locate.replace")}
        </label>
        {report && (
          <p className="panel-note" role="status">
            {[
              t("locate.report.located", { count: report.located }),
              report.kept > 0 && t("locate.report.kept", { count: report.kept }),
              report.unmatched > 0 && t("locate.report.unmatched", { count: report.unmatched }),
              report.noTime > 0 && t("locate.report.noTime", { count: report.noTime }),
              report.unusableSources > 0 &&
                t("locate.report.unusable", { count: report.unusableSources }),
            ]
              .filter(Boolean)
              .join(" · ")}
          </p>
        )}
        <div className="develop-crop-actions">
          <span className="toolbar-spacer" />
          <button type="button" className="button button-small" onClick={close}>
            {t(report ? "import.dismiss" : "develop.crop.cancel")}
          </button>
          <button
            type="submit"
            className="button button-small button-primary"
            disabled={files.length === 0 || running}
          >
            {t("locate.confirm")}
          </button>
        </div>
      </form>
    </div>
  );
}
