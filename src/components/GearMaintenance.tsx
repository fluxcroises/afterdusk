import { CircleAlert, Trash2 } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { type GearRecord, useGear } from "../store/gear";
import {
  MAINTENANCE_KINDS,
  type MaintenanceInput,
  type MaintenanceKind,
  REMINDER_MONTHS,
} from "../store/maintenance";
import { confirmWarning } from "../ui/confirm";
import { formatDay, formatMoney } from "../ui/format";

const CURRENCIES = ["EUR", "USD", "GBP", "CHF", "CAD", "JPY"];

function today(): string {
  const d = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

function AddForm({ record, onDone }: { record: GearRecord; onDone: () => void }) {
  const { t } = useTranslation();
  const [date, setDate] = useState(today);
  const [kind, setKind] = useState<MaintenanceKind>("sensor");
  const [cost, setCost] = useState("");
  const [currency, setCurrency] = useState(record.currency ?? "EUR");
  const [months, setMonths] = useState("");
  const [notes, setNotes] = useState("");
  const submit = async () => {
    const input: MaintenanceInput = {
      date,
      kind,
      costCents: cost === "" ? null : Math.round(Number(cost) * 100),
      currency: cost === "" ? null : currency,
      notes: notes.trim() || null,
      reminderMonths: months === "" ? null : Number(months),
    };
    await useGear.getState().addMaintenance(input);
    onDone();
  };
  return (
    <form
      className="settings-form maintenance-form"
      onSubmit={(e) => {
        e.preventDefault();
        void submit();
      }}
    >
      <label className="field-row">
        <span className="field-row-label">{t("maintenance.date")}</span>
        <input type="date" required value={date} onChange={(e) => setDate(e.currentTarget.value)} />
      </label>
      <label className="field-row">
        <span className="field-row-label">{t("maintenance.kind")}</span>
        <select value={kind} onChange={(e) => setKind(e.currentTarget.value as MaintenanceKind)}>
          {MAINTENANCE_KINDS.map((k) => (
            <option key={k} value={k}>
              {t(`maintenance.kinds.${k}`)}
            </option>
          ))}
        </select>
      </label>
      <label className="field-row">
        <span className="field-row-label">{t("maintenance.cost")}</span>
        <input
          type="number"
          min={0}
          step={0.01}
          value={cost}
          onChange={(e) => setCost(e.currentTarget.value)}
        />
        <select
          aria-label={t("gear.fields.currency")}
          value={currency}
          onChange={(e) => setCurrency(e.currentTarget.value)}
        >
          {CURRENCIES.map((c) => (
            <option key={c} value={c}>
              {c}
            </option>
          ))}
        </select>
      </label>
      <label className="field-row">
        <span className="field-row-label">{t("maintenance.reminder")}</span>
        <select value={months} onChange={(e) => setMonths(e.currentTarget.value)}>
          <option value="">{t("maintenance.noReminder")}</option>
          {REMINDER_MONTHS.map((m) => (
            <option key={m} value={m}>
              {t("maintenance.every", { count: m })}
            </option>
          ))}
        </select>
      </label>
      <label className="gear-notes">
        <span className="field-row-label">{t("gear.fields.notes")}</span>
        <textarea rows={2} value={notes} onChange={(e) => setNotes(e.currentTarget.value)} />
      </label>
      <div className="maintenance-buttons">
        <button type="button" className="button button-small" onClick={onDone}>
          {t("trash.cancel")}
        </button>
        <button type="submit" className="button button-small button-primary">
          {t("maintenance.save")}
        </button>
      </div>
    </form>
  );
}

/** Carte Entretien du tableau de bord : rappels, journal, ajout. */
export function GearMaintenance({ record }: { record: GearRecord }) {
  const { t, i18n } = useTranslation();
  const journal = useGear((s) => s.journal);
  const reminders = useGear((s) => s.reminders);
  const [adding, setAdding] = useState(false);
  const day = (d: string) => formatDay(d, i18n.language);
  const remove = async (id: number, kind: MaintenanceKind, date: string) => {
    const ok = await confirmWarning(
      t("maintenance.confirmDelete", { kind: t(`maintenance.kinds.${kind}`), date: day(date) }),
      {
        title: t("maintenance.title"),
        okLabel: t("gear.delete"),
        cancelLabel: t("trash.cancel"),
      },
    );
    if (ok) await useGear.getState().deleteMaintenance(id);
  };
  return (
    <section className="gear-card maintenance">
      <h2 className="bar-chart-title">{t("maintenance.title")}</h2>
      {reminders.map((r) => (
        <p key={r.kind} className={r.due ? "maintenance-reminder is-due" : "maintenance-reminder"}>
          {r.due && <CircleAlert size={14} aria-hidden="true" />}
          {t(r.due ? "maintenance.dueSince" : "maintenance.dueOn", {
            kind: t(`maintenance.kinds.${r.kind}`),
            date: day(r.dueDate),
          })}
        </p>
      ))}
      {journal.length === 0 && !adding && <p className="panel-note">{t("maintenance.empty")}</p>}
      {journal.length > 0 && (
        <ul className="maintenance-list">
          {journal.map((e) => (
            <li key={e.id}>
              <span className="maintenance-line">
                <span className="maintenance-date">{day(e.date)}</span>
                <span>{t(`maintenance.kinds.${e.kind}`)}</span>
                {e.costCents !== null && (
                  <span className="maintenance-cost">
                    {formatMoney(e.costCents, e.currency, i18n.language)}
                  </span>
                )}
                <button
                  type="button"
                  className="icon-button"
                  aria-label={t("maintenance.delete")}
                  title={t("maintenance.delete")}
                  onClick={() => void remove(e.id, e.kind, e.date)}
                >
                  <Trash2 size={14} aria-hidden="true" />
                </button>
              </span>
              {(e.reminderMonths !== null || e.notes) && (
                <span className="maintenance-detail">
                  {[
                    e.reminderMonths !== null &&
                      t("maintenance.every", { count: e.reminderMonths }),
                    e.notes,
                  ]
                    .filter(Boolean)
                    .join(" · ")}
                </span>
              )}
            </li>
          ))}
        </ul>
      )}
      {adding ? (
        <AddForm record={record} onDone={() => setAdding(false)} />
      ) : (
        <button type="button" className="button button-secondary" onClick={() => setAdding(true)}>
          {t("maintenance.add")}
        </button>
      )}
    </section>
  );
}
