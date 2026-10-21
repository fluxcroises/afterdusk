import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { useTimeFix } from "../store/timefix";
import { formatOffset, formatShift } from "../ui/format";
import { ErrorBanner } from "./ErrorBanner";

/** Décalage de l'ordinateur à cette date, pour les boîtiers sans fuseau. */
function computerOffset(): number {
  return -new Date().getTimezoneOffset();
}

/** Photo > Corriger l'heure… : heure du boîtier ramenée à l'heure locale du lieu. */
export function TimeFixDialog() {
  const ids = useTimeFix((s) => s.ids);
  return ids ? <TimeFixForm count={ids.length} /> : null;
}

function TimeFixForm({ count }: { count: number }) {
  const { t } = useTranslation();
  const clock = useTimeFix((s) => s.clock);
  const offset = useTimeFix((s) => s.offsetMinutes);
  const report = useTimeFix((s) => s.report);
  const corrected = useTimeFix((s) => s.corrected);
  const running = useTimeFix((s) => s.running);
  const error = useTimeFix((s) => s.error);
  const st = useTimeFix.getState();
  const written = clock?.writtenMinutes ?? null;
  const manual = offset ?? written ?? computerOffset();
  const usingWritten = offset === null && written !== null;

  useEffect(() => {
    const escape = (e: KeyboardEvent) => {
      if (e.key === "Escape") useTimeFix.getState().close();
    };
    window.addEventListener("keydown", escape);
    return () => window.removeEventListener("keydown", escape);
  }, []);

  const others = report
    ? [
        report.correct > 0 && t("timefix.correct", { count: report.correct }),
        report.noPlace > 0 && t("timefix.noPlace", { count: report.noPlace }),
        report.noZone > 0 && t("timefix.noZone", { count: report.noZone }),
        report.noTime > 0 && t("locate.report.noTime", { count: report.noTime }),
      ].filter(Boolean)
    : [];

  return (
    <div className="modal-backdrop" onMouseDown={(e) => e.target === e.currentTarget && st.close()}>
      <form
        className="copy-dialog locate-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="timefix-title"
        onSubmit={(e) => {
          e.preventDefault();
          void st.apply();
        }}
      >
        <h2 id="timefix-title" className="copy-title">
          {t("timefix.title", { count })}
        </h2>
        <ErrorBanner error={error} onDismiss={() => useTimeFix.setState({ error: null })} />
        <fieldset className="form-fieldset locate-clock">
          <legend className="field-row-label">{t("locate.clock")}</legend>
          <label className="check-row">
            <input
              type="radio"
              name="timefix-clock"
              checked={usingWritten}
              disabled={written === null}
              onChange={() => void st.setOffset(null)}
            />
            {written !== null && clock?.mixed
              ? t("locate.writtenMixed", { count: clock.withOffset, total: clock.total })
              : written !== null
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
              name="timefix-clock"
              checked={!usingWritten}
              onChange={() => void st.setOffset(manual)}
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
              onChange={(e) => void st.setOffset(Math.round(Number(e.currentTarget.value) * 60))}
            />
            {t("locate.hours")}
          </label>
        </fieldset>
        {report && report.groups.length > 0 && (
          <ul className="timefix-groups">
            {report.groups.map((g) => (
              <li key={`${g.city}|${g.cameraMinutes}|${g.localMinutes}`}>
                {t("timefix.group", {
                  count: g.count,
                  city: g.city ?? t("timefix.unnamed"),
                  camera: formatOffset(g.cameraMinutes),
                  local: formatOffset(g.localMinutes),
                  shift: formatShift(g.shiftMinutes, t("locate.hours")),
                })}
              </li>
            ))}
          </ul>
        )}
        {report && report.groups.length === 0 && corrected === null && (
          <p className="panel-note">{t("timefix.nothing")}</p>
        )}
        {others.length > 0 && <p className="panel-note">{others.join(" · ")}</p>}
        {corrected !== null && (
          <p className="panel-note" role="status">
            {t("timefix.done", { count: corrected })}
          </p>
        )}
        <div className="develop-crop-actions">
          <span className="toolbar-spacer" />
          <button type="button" className="button button-small" onClick={st.close}>
            {t(corrected !== null ? "import.dismiss" : "develop.crop.cancel")}
          </button>
          <button
            type="submit"
            className="button button-small button-primary"
            disabled={!report || report.groups.length === 0 || running}
          >
            {t("timefix.confirm")}
          </button>
        </div>
      </form>
    </div>
  );
}
