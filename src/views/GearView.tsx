import { invoke } from "@tauri-apps/api/core";
import { open as chooseFile, save as chooseTarget } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { previewUrl } from "../api/previews";
import { BarChart } from "../components/BarChart";
import { EmptyState } from "../components/EmptyState";
import { ErrorBanner } from "../components/ErrorBanner";
import { GearMaintenance } from "../components/GearMaintenance";
import { GearSilhouette } from "../components/GearSilhouette";
import { FieldRow, ListRow, PanelNote, PanelSection } from "../components/PanelSection";
import { ScreenLayout } from "../components/ScreenLayout";
import { Thumbnail } from "../components/Thumbnail";
import {
  GEAR_KINDS,
  type GearKind,
  type GearRecord,
  type GearUpdate,
  useGear,
} from "../store/gear";
import { toCommandError } from "../api/core";
import { useImport } from "../store/import";
import { useLibrary } from "../store/library";
import { useReminders } from "../store/maintenance";
import { confirmWarning } from "../ui/confirm";
import { formatCaptureTime } from "../ui/format";
import { apertureBars, focalBars, isoBars, monthBars } from "../ui/gearStats";
import { collectGear, csvDelimiter, inventoryCsv, inventoryPdf } from "../ui/gearExport";
import { Camera } from "lucide-react";

const CURRENCIES = ["EUR", "USD", "GBP", "CHF", "CAD", "JPY"];
const CONDITIONS = ["new", "excellent", "good", "worn", "repair", "broken"];
const BODY_STYLES = ["mirrorless", "dslr", "compact"];
const LENS_STYLES = ["prime", "zoom"];

/** Nom affiché ; un accessoire tout juste ajouté prend le nom de son type. */
function useGearName() {
  const { t } = useTranslation();
  return (g: Pick<GearRecord, "displayName" | "kind">) =>
    g.displayName.trim() || t(`gear.kinds.${g.kind}`);
}

function group(kind: GearKind): "bodies" | "lenses" | "accessories" {
  return kind === "body" ? "bodies" : kind === "lens" ? "lenses" : "accessories";
}

function GearList() {
  const { t } = useTranslation();
  const items = useGear((s) => s.items);
  const selected = useGear((s) => s.selectedId);
  const showHidden = useGear((s) => s.showHidden);
  const nameOf = useGearName();
  const reminders = useReminders((s) => s.reminders);
  const due = new Set(reminders.filter((r) => r.due).map((r) => r.gearId));
  const st = useGear.getState();
  return (
    <>
      {(["bodies", "lenses", "accessories"] as const).map((g) => {
        const list = items.filter((i) => group(i.kind) === g);
        return (
          <PanelSection key={g} id={`gear.${g}`} title={t(`gear.groups.${g}`)}>
            {list.length === 0 ? (
              <PanelNote>{t(`gear.empty.${g}`)}</PanelNote>
            ) : (
              list.map((i) => (
                <ListRow
                  key={i.id}
                  label={nameOf(i)}
                  hint={
                    i.hidden
                      ? t("gear.hiddenHint")
                      : due.has(i.id)
                        ? t("maintenance.dueHint")
                        : undefined
                  }
                  count={i.autoDetected ? i.photoCount : undefined}
                  selected={i.id === selected}
                  onClick={() => void st.select(i.id)}
                />
              ))
            )}
          </PanelSection>
        );
      })}
      <div className="gear-list-options">
        <label className="check-row">
          <input
            type="checkbox"
            checked={showHidden}
            onChange={(e) => void st.setShowHidden(e.currentTarget.checked)}
          />
          {t("gear.showHidden")}
        </label>
        <select
          aria-label={t("gear.add")}
          value=""
          onChange={(e) => {
            const kind = e.currentTarget.value as GearKind;
            if (kind) void st.create(kind);
          }}
        >
          <option value="">{t("gear.add")}</option>
          {GEAR_KINDS.map((k) => (
            <option key={k} value={k}>
              {t(`gear.kinds.${k}`)}
            </option>
          ))}
        </select>
      </div>
    </>
  );
}

/** Photo de l'appareil, sinon sa silhouette, et la marque en toutes lettres. */
function GearPicture({ record }: { record: GearRecord }) {
  const photoUrl = useGear((s) => s.photoUrl);
  const src =
    photoUrl ?? (record.photoMediaId !== null ? previewUrl(record.photoMediaId, 1024) : null);
  return (
    <div className="gear-picture">
      {src ? (
        <span className="gear-photo">
          <Thumbnail src={src} />
        </span>
      ) : (
        <GearSilhouette kind={record.kind} style={record.style} />
      )}
      {record.make && <span className="gear-brand">{record.make}</span>}
    </div>
  );
}

function TextField({
  label,
  value,
  onSave,
  type = "text",
}: {
  label: string;
  value: string | null;
  onSave: (value: string | null) => void;
  type?: "text" | "date";
}) {
  const [draft, setDraft] = useState(value ?? "");
  return (
    <label className="field-row">
      <span className="field-row-label">{label}</span>
      <input
        type={type}
        value={draft}
        onChange={(e) => {
          setDraft(e.currentTarget.value);
          if (type === "date") onSave(e.currentTarget.value || null);
        }}
        onBlur={() => {
          if (type !== "date" && draft !== (value ?? "")) onSave(draft || null);
        }}
      />
    </label>
  );
}

function GearForm({ record }: { record: GearRecord }) {
  const { t } = useTranslation();
  const save = (update: GearUpdate) => void useGear.getState().save(update);
  const manual = !record.autoDetected;
  const styles = record.kind === "body" ? BODY_STYLES : record.kind === "lens" ? LENS_STYLES : [];
  const [price, setPrice] = useState(
    record.priceCents === null ? "" : String(record.priceCents / 100),
  );
  return (
    <form className="settings-form gear-form" onSubmit={(e) => e.preventDefault()}>
      <TextField
        label={t("gear.fields.name")}
        value={record.name}
        onSave={(name) => save({ name })}
      />
      {manual ? (
        <>
          <label className="field-row">
            <span className="field-row-label">{t("gear.fields.kind")}</span>
            <select
              value={record.kind}
              onChange={(e) => save({ kind: e.currentTarget.value as GearKind })}
            >
              {GEAR_KINDS.map((k) => (
                <option key={k} value={k}>
                  {t(`gear.kinds.${k}`)}
                </option>
              ))}
            </select>
          </label>
          <TextField
            label={t("gear.fields.make")}
            value={record.make}
            onSave={(make) => save({ make: make ?? "" })}
          />
          <TextField
            label={t("gear.fields.model")}
            value={record.model}
            onSave={(model) => save({ model: model ?? "" })}
          />
        </>
      ) : (
        <>
          <FieldRow label={t("gear.fields.kind")} value={t(`gear.kinds.${record.kind}`)} />
          <FieldRow label={t("gear.fields.make")} value={record.make || undefined} />
          <FieldRow label={t("gear.fields.model")} value={record.model} />
        </>
      )}
      {styles.length > 0 && (
        <label className="field-row">
          <span className="field-row-label">{t("gear.fields.style")}</span>
          <select
            value={record.style ?? ""}
            onChange={(e) => save({ style: e.currentTarget.value as GearRecord["style"] })}
          >
            {styles.map((s) => (
              <option key={s} value={s}>
                {t(`gear.styles.${s}`)}
              </option>
            ))}
          </select>
        </label>
      )}
      <TextField
        label={t("gear.fields.serial")}
        value={record.serial}
        onSave={(serial) => save({ serial })}
      />
      <TextField
        label={t("gear.fields.purchaseDate")}
        type="date"
        value={record.purchaseDate}
        onSave={(purchaseDate) => save({ purchaseDate })}
      />
      <TextField
        label={t("gear.fields.purchasePlace")}
        value={record.purchasePlace}
        onSave={(purchasePlace) => save({ purchasePlace })}
      />
      <label className="field-row">
        <span className="field-row-label">{t("gear.fields.price")}</span>
        <input
          type="number"
          min={0}
          step={0.01}
          value={price}
          onChange={(e) => setPrice(e.currentTarget.value)}
          onBlur={() => save({ priceCents: price === "" ? null : Math.round(Number(price) * 100) })}
        />
        <select
          aria-label={t("gear.fields.currency")}
          value={record.currency ?? "EUR"}
          onChange={(e) => save({ currency: e.currentTarget.value })}
        >
          {CURRENCIES.map((c) => (
            <option key={c} value={c}>
              {c}
            </option>
          ))}
        </select>
      </label>
      <label className="field-row">
        <span className="field-row-label">{t("gear.fields.condition")}</span>
        <select
          value={record.condition ?? ""}
          onChange={(e) => save({ condition: e.currentTarget.value || null })}
        >
          <option value="">{t("info.notSet")}</option>
          {CONDITIONS.map((c) => (
            <option key={c} value={c}>
              {t(`gear.conditions.${c}`)}
            </option>
          ))}
        </select>
      </label>
      <TextField
        label={t("gear.fields.warrantyUntil")}
        type="date"
        value={record.warrantyUntil}
        onSave={(warrantyUntil) => save({ warrantyUntil })}
      />
      <label className="gear-notes">
        <span className="field-row-label">{t("gear.fields.notes")}</span>
        <NotesField record={record} />
      </label>
    </form>
  );
}

function NotesField({ record }: { record: GearRecord }) {
  const [draft, setDraft] = useState(record.notes ?? "");
  return (
    <textarea
      rows={4}
      value={draft}
      onChange={(e) => setDraft(e.currentTarget.value)}
      onBlur={() => {
        if (draft !== (record.notes ?? "")) void useGear.getState().save({ notes: draft || null });
      }}
    />
  );
}

function GearStatsView({ record }: { record: GearRecord }) {
  const { t, i18n } = useTranslation();
  const stats = useGear((s) => s.stats);
  if (!stats || stats.photos === 0) return null;
  const number = new Intl.NumberFormat(i18n.language);
  const photos = (count: number) => t("gear.stats.photoCount", { count, n: number.format(count) });
  return (
    <>
      <h2 className="visually-hidden">{t("gear.stats.title")}</h2>
      <BarChart title={t("gear.stats.focal")} bars={focalBars(stats.focal)} />
      <BarChart title={t("gear.stats.aperture")} bars={apertureBars(stats.aperture)} />
      <BarChart title={t("gear.stats.iso")} bars={isoBars(stats.iso)} />
      <BarChart title={t("gear.stats.months")} bars={monthBars(stats.months, i18n.language)} />
      {stats.partners.length > 0 && (
        <div className="gear-stats-list">
          <h3 className="bar-chart-title">
            {t(record.kind === "body" ? "gear.stats.lenses" : "gear.stats.bodies")}
          </h3>
          {stats.partners.map((p) => (
            <ListRow
              key={p.id}
              label={p.name}
              count={p.count}
              onClick={() => void useGear.getState().select(p.id)}
            />
          ))}
        </div>
      )}
      {stats.albums.length > 0 && (
        <div className="gear-stats-list">
          <h3 className="bar-chart-title">{t("gear.stats.albums")}</h3>
          <table className="gear-stats-table">
            <tbody>
              {stats.albums.map((a) => (
                <tr key={a.albumId}>
                  <th scope="row">{a.name}</th>
                  <td>{photos(a.count)}</td>
                  {record.kind === "body" && <td>{a.favoriteLens ?? ""}</td>}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </>
  );
}

function ShutterRow() {
  const { t, i18n } = useTranslation();
  const shutter = useGear((s) => s.stats?.shutter);
  if (!shutter) return null;
  const number = new Intl.NumberFormat(i18n.language);
  return (
    <>
      <FieldRow
        label={t("gear.stats.shutter")}
        value={
          shutter.read === null
            ? t("gear.stats.shutterAtLeast", { n: number.format(shutter.estimate) })
            : number.format(shutter.estimate)
        }
      />
      <PanelNote>
        {shutter.read === null
          ? t("gear.stats.shutterEstimated")
          : t(shutter.since === 0 ? "gear.stats.shutterReadLatest" : "gear.stats.shutterRead", {
              count: shutter.since,
              n: number.format(shutter.read),
              date: formatCaptureTime(shutter.readAt, i18n.language) ?? "",
            })}
      </PanelNote>
    </>
  );
}

function GearActions({ record }: { record: GearRecord }) {
  const { t, i18n } = useTranslation();
  const items = useGear((s) => s.items);
  const selectedPhoto = useLibrary((s) =>
    s.selectedId !== null && s.selectedId > 0 ? s.selectedId : null,
  );
  const [target, setTarget] = useState<number | "">("");
  const st = useGear.getState();
  const others = items.filter((i) => i.kind === record.kind && i.id !== record.id);
  const nameOf = useGearName();
  const remove = async () => {
    const ok = await confirmWarning(t("gear.confirmDelete", { name: nameOf(record) }), {
      title: t("modules.gear"),
      okLabel: t("gear.delete"),
      cancelLabel: t("trash.cancel"),
    });
    if (ok) await st.remove();
  };
  const pick = async () => {
    const chosen = await chooseFile({
      multiple: false,
      title: t("gear.chooseImage"),
      filters: [{ name: t("gear.images"), extensions: ["jpg", "jpeg", "png", "webp"] }],
    });
    if (typeof chosen === "string") await st.setPhotoFile(chosen);
  };
  return (
    <>
      <PanelSection id="gear.usage" title={t("gear.usage")}>
        {record.autoDetected ? (
          <>
            <FieldRow label={t("gear.photos")} value={String(record.photoCount)} />
            <FieldRow
              label={t("gear.firstUsed")}
              value={formatCaptureTime(record.firstUsed, i18n.language) ?? undefined}
            />
            <FieldRow
              label={t("gear.lastUsed")}
              value={formatCaptureTime(record.lastUsed, i18n.language) ?? undefined}
            />
            <ShutterRow />
          </>
        ) : (
          <PanelNote>{t("gear.manual")}</PanelNote>
        )}
      </PanelSection>
      {record.aliases.length > 0 && (
        <PanelSection id="gear.aliases" title={t("gear.aliases")}>
          {record.aliases.map((a) => (
            <p key={a} className="gear-alias">
              {a}
            </p>
          ))}
        </PanelSection>
      )}
      <PanelSection id="gear.photo" title={t("gear.photo")}>
        <div className="gear-actions">
          <button type="button" className="button button-secondary" onClick={() => void pick()}>
            {t("gear.chooseImage")}
          </button>
          <button
            type="button"
            className="button button-secondary"
            disabled={selectedPhoto === null}
            onClick={() => void st.setPhotoMedia(selectedPhoto)}
          >
            {t("gear.useSelected")}
          </button>
          {(record.photoMediaId !== null || record.hasPhotoFile) && (
            <button
              type="button"
              className="button button-small"
              onClick={() => void st.setPhotoMedia(null)}
            >
              {t("gear.removePhoto")}
            </button>
          )}
        </div>
      </PanelSection>
      <PanelSection id="gear.manage" title={t("gear.manage")}>
        <div className="gear-actions">
          <label className="check-row">
            <input
              type="checkbox"
              checked={record.confirmed}
              onChange={(e) => void st.setConfirmed(e.currentTarget.checked)}
            />
            {t("gear.confirmed")}
          </label>
          <button
            type="button"
            className="button button-secondary"
            onClick={() => void st.setHidden(!record.hidden)}
          >
            {t(record.hidden ? "gear.unhide" : "gear.hide")}
          </button>
          {others.length > 0 && (
            <div className="gear-merge">
              <select
                aria-label={t("gear.mergeWith")}
                value={target}
                onChange={(e) =>
                  setTarget(e.currentTarget.value ? Number(e.currentTarget.value) : "")
                }
              >
                <option value="">{t("gear.mergeWith")}</option>
                {others.map((o) => (
                  <option key={o.id} value={o.id}>
                    {nameOf(o)}
                  </option>
                ))}
              </select>
              <button
                type="button"
                className="button button-small"
                disabled={target === ""}
                onClick={() => {
                  if (target !== "") void st.merge(target);
                  setTarget("");
                }}
              >
                {t("gear.merge")}
              </button>
            </div>
          )}
          {!record.autoDetected && (
            <button type="button" className="button button-small" onClick={() => void remove()}>
              {t("gear.delete")}
            </button>
          )}
        </div>
      </PanelSection>
    </>
  );
}

/** Export de l'inventaire (matériel visible) en PDF ou en CSV. */
function ExportButtons() {
  const { t, i18n } = useTranslation();
  const items = useGear((s) => s.items);
  const library = useImport((s) => s.libraries.find((l) => l.id === s.libraryId)?.name ?? "");
  const [busy, setBusy] = useState(false);
  const run = async (format: "pdf" | "csv") => {
    const path = await chooseTarget({
      title: t(`gearExport.${format}`),
      defaultPath: `${t("gearExport.fileName")}.${format}`,
      filters: [{ name: format.toUpperCase(), extensions: [format] }],
    });
    if (!path) return;
    setBusy(true);
    try {
      const data = await collectGear(items);
      if (format === "csv") {
        await invoke("gear_export_csv", {
          path,
          table: inventoryCsv(t, data, i18n.language),
          delimiter: csvDelimiter(i18n.language),
        });
      } else {
        await invoke("gear_export_pdf", {
          path,
          doc: inventoryPdf(t, data, i18n.language, library),
        });
      }
    } catch (e) {
      useGear.setState({ error: toCommandError(e) });
    } finally {
      setBusy(false);
    }
  };
  return (
    <>
      <button
        type="button"
        className="button button-small"
        disabled={busy || items.length === 0}
        onClick={() => void run("pdf")}
      >
        {t("gearExport.pdf")}
      </button>
      <button
        type="button"
        className="button button-small"
        disabled={busy || items.length === 0}
        onClick={() => void run("csv")}
      >
        {t("gearExport.csv")}
      </button>
    </>
  );
}

export function GearView() {
  const { t } = useTranslation();
  const record = useGear((s) => s.record);
  const error = useGear((s) => s.error);
  const count = useGear((s) => s.items.length);
  const nameOf = useGearName();

  useEffect(() => {
    void useGear.getState().load();
  }, []);

  return (
    <ScreenLayout
      left={<GearList />}
      right={record ? <GearActions record={record} /> : undefined}
      toolbar={
        <>
          <span className="toolbar-status">{t("gear.count", { count })}</span>
          <span className="toolbar-spacer" />
          <ExportButtons />
        </>
      }
    >
      <ErrorBanner error={error} onDismiss={() => useGear.setState({ error: null })} />
      {record ? (
        <div className="gear-dashboard">
          <div className="gear-card gear-identity">
            <GearPicture record={record} />
            <h1 className="gear-title">{nameOf(record)}</h1>
          </div>
          <div className="gear-card gear-form-card">
            <GearForm key={record.id} record={record} />
          </div>
          <GearMaintenance record={record} />
          <GearStatsView record={record} />
        </div>
      ) : (
        <EmptyState icon={Camera} title={t("gear.emptyTitle")} />
      )}
    </ScreenLayout>
  );
}
