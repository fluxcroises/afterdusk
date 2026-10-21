import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, SlidersHorizontal, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { pickFolder } from "../actions";
import {
  ANCHORS,
  BUILTIN_EXPORTS,
  type ExportOutput,
  type ExportPreset,
  type Resize,
  type ResizeMode,
  type Watermark,
  useDeliver,
} from "../store/deliver";
import { useLibrary } from "../store/library";
import { pasteTargets } from "../ui/paste";
import { Slider } from "./DevelopPanels";
import { IconButton } from "./IconButton";
import { PanelSection } from "./PanelSection";

/** Nom affiché d'un preset d'export. */
function usePresetName() {
  const { t } = useTranslation();
  return (p: ExportPreset) => p.name ?? t(`deliver.presets.${p.id}`, { defaultValue: p.id });
}

/** Résumé d'un preset : format, taille, espace. */
function useSummary() {
  const { t } = useTranslation();
  return (p: ExportPreset) => {
    const o = p.output;
    const format =
      o.kind === "original"
        ? t("deliver.output.original")
        : o.kind === "jpeg" || o.kind === "avif"
          ? `${t(`deliver.output.${o.kind}`)} ${o.quality}`
          : `${t(`deliver.output.${o.kind}`)} ${o.sixteen ? 16 : 8} ${t("deliver.bits")}`;
    if (o.kind === "original") return format;
    const r = p.resize;
    const size =
      r.mode === "full"
        ? t("deliver.resize.fullShort")
        : r.mode === "percent"
          ? `${r.value} %`
          : r.mode === "fit"
            ? `${r.value[0]} × ${r.value[1]}`
            : `${r.value} px`;
    return `${format} · ${size} · ${t(`deliver.color.${p.color}`)}`;
  };
}

const DEFAULT_RESIZE: Record<ResizeMode, Resize> = {
  full: { mode: "full" },
  longSide: { mode: "longSide", value: 2048 },
  width: { mode: "width", value: 2048 },
  height: { mode: "height", value: 1365 },
  percent: { mode: "percent", value: 50 },
  fit: { mode: "fit", value: [1080, 1350] },
};

const DEFAULT_OUTPUT: Record<ExportOutput["kind"], ExportOutput> = {
  original: { kind: "original" },
  jpeg: { kind: "jpeg", quality: 90 },
  png: { kind: "png", sixteen: false },
  tiff: { kind: "tiff", sixteen: true },
  avif: { kind: "avif", quality: 70 },
};

function NumberField({
  label,
  value,
  min,
  max,
  onChange,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  onChange: (v: number) => void;
}) {
  return (
    <label className="deliver-number">
      <span>{label}</span>
      <input
        type="number"
        min={min}
        max={max}
        value={value}
        onChange={(e) => {
          const v = Number(e.currentTarget.value);
          if (Number.isFinite(v)) onChange(Math.min(max, Math.max(min, Math.round(v))));
        }}
      />
    </label>
  );
}

/** Options d'un preset d'export, dépliées sous sa ligne. */
function PresetEditor({ preset }: { preset: ExportPreset }) {
  const { t } = useTranslation();
  const nameOf = usePresetName();
  const st = useDeliver.getState();
  const set = (patch: Partial<ExportPreset>) => st.updatePreset(preset.id, patch);
  const o = preset.output;
  const r = preset.resize;
  const original = o.kind === "original";
  const builtin = BUILTIN_EXPORTS.includes(preset.id);

  return (
    <div className="deliver-editor">
      {!builtin && (
        <label className="deliver-field">
          <span>{t("deliver.name")}</span>
          <input
            type="text"
            value={preset.name ?? ""}
            maxLength={60}
            onChange={(e) => set({ name: e.currentTarget.value })}
          />
        </label>
      )}
      <label className="deliver-field">
        <span>{t("deliver.format")}</span>
        <select
          value={o.kind}
          onChange={(e) =>
            set({ output: DEFAULT_OUTPUT[e.currentTarget.value as ExportOutput["kind"]] })
          }
        >
          {(Object.keys(DEFAULT_OUTPUT) as ExportOutput["kind"][]).map((k) => (
            <option key={k} value={k}>
              {t(`deliver.output.${k}`)}
            </option>
          ))}
        </select>
      </label>
      {(o.kind === "jpeg" || o.kind === "avif") && (
        <Slider
          label={t("deliver.quality")}
          display={String(o.quality)}
          value={o.quality}
          min={1}
          max={100}
          step={1}
          onChange={(v) => set({ output: { ...o, quality: Math.round(v) } })}
          onReset={() => set({ output: DEFAULT_OUTPUT[o.kind] })}
        />
      )}
      {(o.kind === "png" || o.kind === "tiff") && (
        <label className="check-row">
          <input
            type="checkbox"
            checked={o.sixteen}
            onChange={(e) => set({ output: { ...o, sixteen: e.currentTarget.checked } })}
          />
          {t("deliver.sixteen")}
        </label>
      )}
      {original ? (
        <p className="panel-note">{t("deliver.originalHint")}</p>
      ) : (
        <>
          <label className="deliver-field">
            <span>{t("deliver.size")}</span>
            <select
              value={r.mode}
              onChange={(e) => set({ resize: DEFAULT_RESIZE[e.currentTarget.value as ResizeMode] })}
            >
              {(Object.keys(DEFAULT_RESIZE) as ResizeMode[]).map((m) => (
                <option key={m} value={m}>
                  {t(`deliver.resize.${m}`)}
                </option>
              ))}
            </select>
          </label>
          {(r.mode === "longSide" || r.mode === "width" || r.mode === "height") && (
            <NumberField
              label={t("deliver.pixels")}
              value={r.value}
              min={16}
              max={20000}
              onChange={(v) => set({ resize: { ...r, value: v } })}
            />
          )}
          {r.mode === "percent" && (
            <NumberField
              label={t("deliver.percent")}
              value={r.value}
              min={1}
              max={100}
              onChange={(v) => set({ resize: { ...r, value: v } })}
            />
          )}
          {r.mode === "fit" && (
            <div className="deliver-pair">
              <NumberField
                label={t("deliver.width")}
                value={r.value[0]}
                min={16}
                max={20000}
                onChange={(v) => set({ resize: { mode: "fit", value: [v, r.value[1]] } })}
              />
              <NumberField
                label={t("deliver.height")}
                value={r.value[1]}
                min={16}
                max={20000}
                onChange={(v) => set({ resize: { mode: "fit", value: [r.value[0], v] } })}
              />
            </div>
          )}
          <label className="deliver-field">
            <span>{t("deliver.colorSpace")}</span>
            <select
              value={o.kind === "avif" ? "srgb" : preset.color}
              disabled={o.kind === "avif"}
              onChange={(e) => set({ color: e.currentTarget.value as ExportPreset["color"] })}
            >
              {(["srgb", "displayP3", "adobeRgb"] as const).map((c) => (
                <option key={c} value={c}>
                  {t(`deliver.color.${c}`)}
                </option>
              ))}
            </select>
          </label>
          <label className="deliver-field">
            <span>{t("deliver.sharpen")}</span>
            <select
              value={preset.sharpen}
              onChange={(e) => set({ sharpen: e.currentTarget.value as ExportPreset["sharpen"] })}
            >
              {(["none", "screen", "paper"] as const).map((s) => (
                <option key={s} value={s}>
                  {t(`deliver.sharpening.${s}`)}
                </option>
              ))}
            </select>
          </label>
          <label className="deliver-field">
            <span>{t("deliver.metadata")}</span>
            <select
              value={preset.metadata}
              onChange={(e) => set({ metadata: e.currentTarget.value as ExportPreset["metadata"] })}
            >
              {(["all", "noGps", "noGpsSerial", "copyrightOnly", "none"] as const).map((m) => (
                <option key={m} value={m}>
                  {t(`deliver.metadataPolicy.${m}`)}
                </option>
              ))}
            </select>
          </label>
          <label className="check-row">
            <input
              type="checkbox"
              checked={preset.watermark}
              onChange={(e) => set({ watermark: e.currentTarget.checked })}
            />
            {t("deliver.useWatermark")}
          </label>
        </>
      )}
      <label className="deliver-field">
        <span>{t("deliver.naming")}</span>
        <input
          type="text"
          value={preset.naming}
          maxLength={120}
          onChange={(e) => set({ naming: e.currentTarget.value })}
        />
      </label>
      <p className="panel-note">{t("deliver.namingHint")}</p>
      <div className="develop-crop-actions">
        <button
          type="button"
          className="button button-small"
          onClick={() =>
            st.duplicatePreset(preset.id, t("deliver.copyName", { name: nameOf(preset) }))
          }
        >
          {t("deliver.duplicate")}
        </button>
        <span className="toolbar-spacer" />
        <button
          type="button"
          className="button button-small"
          onClick={() => void st.removePreset(preset.id)}
        >
          {t(builtin ? "deliver.restore" : "deliver.delete")}
        </button>
      </div>
    </div>
  );
}

/** Presets d'export : cocher ceux à produire, déplier les options d'un preset. */
export function ExportPresetsSection() {
  const { t } = useTranslation();
  const settings = useDeliver((s) => s.settings);
  const chosen = useDeliver((s) => s.chosen);
  const editing = useDeliver((s) => s.editing);
  const nameOf = usePresetName();
  const summary = useSummary();
  const st = useDeliver.getState();
  return (
    <PanelSection id="deliver.presets" title={t("deliver.presetsTitle")}>
      <ul className="deliver-presets">
        {settings?.presets.map((p) => (
          <li key={p.id} className="deliver-preset" data-open={editing === p.id || undefined}>
            <div className="deliver-preset-row">
              <label className="check-row deliver-preset-check">
                <input
                  type="checkbox"
                  checked={chosen.includes(p.id)}
                  onChange={() => st.toggleChosen(p.id)}
                />
                <span className="deliver-preset-text">
                  <span>{nameOf(p)}</span>
                  <span className="deliver-preset-summary">{summary(p)}</span>
                </span>
              </label>
              <IconButton
                icon={SlidersHorizontal}
                label={t("deliver.options", { name: nameOf(p) })}
                pressed={editing === p.id}
                onClick={() => st.setEditing(p.id)}
              />
            </div>
            {editing === p.id && <PresetEditor preset={p} />}
          </li>
        ))}
      </ul>
      {chosen.length > 1 && <p className="panel-note">{t("deliver.subfolders")}</p>}
    </PanelSection>
  );
}

const NEW_WATERMARK: Watermark = {
  content: { kind: "text", text: "©", dark: false },
  anchor: "bottomRight",
  size: 20,
  opacity: 60,
  margin: 3,
};

/** Filigrane : texte ou logo, position, taille, opacité, marge. */
export function WatermarkSection() {
  const { t } = useTranslation();
  const settings = useDeliver((s) => s.settings);
  const wm = settings?.watermark ?? null;
  const st = useDeliver.getState();
  const set = (patch: Partial<Watermark> | null) => {
    if (!settings) return;
    st.update({
      ...settings,
      watermark: patch === null ? null : { ...(wm ?? NEW_WATERMARK), ...patch },
    });
  };
  return (
    <PanelSection id="deliver.watermark" title={t("deliver.watermark")} defaultCollapsed>
      {!wm ? (
        <>
          <button
            type="button"
            className="button button-small"
            disabled={!settings}
            onClick={() => set({})}
          >
            {t("deliver.addWatermark")}
          </button>
        </>
      ) : (
        <div className="deliver-editor">
          <div className="develop-segmented" role="radiogroup" aria-label={t("deliver.watermark")}>
            {(["text", "image"] as const).map((k) => (
              <button
                key={k}
                type="button"
                role="radio"
                aria-checked={wm.content.kind === k}
                onClick={() =>
                  set({
                    content:
                      k === "text"
                        ? { kind: "text", text: "©", dark: false }
                        : { kind: "image", path: "" },
                  })
                }
              >
                {t(`deliver.watermarkKind.${k}`)}
              </button>
            ))}
          </div>
          {wm.content.kind === "text" ? (
            <>
              <label className="deliver-field">
                <span>{t("deliver.watermarkText")}</span>
                <input
                  type="text"
                  value={wm.content.text}
                  maxLength={120}
                  onChange={(e) =>
                    set({
                      content: {
                        kind: "text",
                        text: e.currentTarget.value,
                        dark: wm.content.kind === "text" && wm.content.dark,
                      },
                    })
                  }
                />
              </label>
              <label className="check-row">
                <input
                  type="checkbox"
                  checked={wm.content.dark}
                  onChange={(e) =>
                    wm.content.kind === "text" &&
                    set({ content: { ...wm.content, dark: e.currentTarget.checked } })
                  }
                />
                {t("deliver.watermarkDark")}
              </label>
            </>
          ) : (
            <div className="deliver-path">
              <span className="deliver-path-value" title={wm.content.path}>
                {wm.content.path || t("deliver.noLogo")}
              </span>
              <button
                type="button"
                className="button button-small"
                onClick={async () => {
                  const path = await open({
                    multiple: false,
                    filters: [{ name: "PNG", extensions: ["png"] }],
                  });
                  if (typeof path === "string") set({ content: { kind: "image", path } });
                }}
              >
                {t("deliver.choose")}
              </button>
            </div>
          )}
          <div className="deliver-anchors" role="radiogroup" aria-label={t("deliver.position")}>
            {ANCHORS.map((a) => (
              <button
                key={a}
                type="button"
                role="radio"
                aria-checked={wm.anchor === a}
                aria-label={t(`deliver.anchors.${a}`)}
                title={t(`deliver.anchors.${a}`)}
                onClick={() => set({ anchor: a })}
              />
            ))}
          </div>
          <Slider
            label={t("deliver.watermarkSize")}
            display={`${wm.size} %`}
            value={wm.size}
            min={1}
            max={100}
            step={1}
            onChange={(v) => set({ size: Math.round(v) })}
            onReset={() => set({ size: NEW_WATERMARK.size })}
          />
          <Slider
            label={t("deliver.opacity")}
            display={`${wm.opacity} %`}
            value={wm.opacity}
            min={0}
            max={100}
            step={1}
            onChange={(v) => set({ opacity: Math.round(v) })}
            onReset={() => set({ opacity: NEW_WATERMARK.opacity })}
          />
          <Slider
            label={t("deliver.margin")}
            display={`${wm.margin} %`}
            value={wm.margin}
            min={0}
            max={25}
            step={0.5}
            onChange={(v) => set({ margin: v })}
            onReset={() => set({ margin: NEW_WATERMARK.margin })}
          />
          <p className="panel-note">{t("deliver.watermarkPerPreset")}</p>
          <button type="button" className="button button-small" onClick={() => set(null)}>
            {t("deliver.removeWatermark")}
          </button>
        </div>
      )}
    </PanelSection>
  );
}

/** Destination : dossier, et ouverture à la fin. */
export function DestinationSection() {
  const { t } = useTranslation();
  const settings = useDeliver((s) => s.settings);
  const st = useDeliver.getState();
  return (
    <PanelSection id="deliver.destination" title={t("deliver.destination")}>
      <div className="deliver-path">
        <span className="deliver-path-value" title={settings?.destination ?? undefined}>
          {settings?.destination ?? t("deliver.noDestination")}
        </span>
        <button
          type="button"
          className="button button-small"
          disabled={!settings}
          onClick={async () => {
            const path = await pickFolder(t("deliver.chooseDestination"));
            if (path && settings) st.update({ ...settings, destination: path });
          }}
        >
          {t("deliver.choose")}
        </button>
      </div>
      <label className="check-row">
        <input
          type="checkbox"
          checked={settings?.openFolder ?? true}
          onChange={(e) =>
            settings && st.update({ ...settings, openFolder: e.currentTarget.checked })
          }
        />
        {t("deliver.openFolder")}
      </label>
    </PanelSection>
  );
}

/** Bouton principal : exporter la sélection avec les presets cochés. */
export function ExportButton() {
  const { t } = useTranslation();
  const settings = useDeliver((s) => s.settings);
  const chosen = useDeliver((s) => s.chosen);
  const running = useDeliver((s) => s.running);
  const count = useLibrary((s) =>
    s.selectedIds.length > 1 ? s.selectedIds.length : s.selectedId === null ? 0 : 1,
  );
  const nameOf = usePresetName();
  const reason = !settings?.destination
    ? t("deliver.needDestination")
    : chosen.length === 0
      ? t("deliver.needPreset")
      : count === 0
        ? t("deliver.needSelection")
        : null;
  return (
    <div className="deliver-footer">
      {reason && <p className="panel-note">{reason}</p>}
      <button
        type="button"
        className="button button-primary deliver-export"
        disabled={running || reason !== null}
        onClick={() => {
          const folders = Object.fromEntries(
            (settings?.presets ?? []).map((p) => [p.id, nameOf(p)]),
          );
          void useDeliver.getState().start(pasteTargets(), folders);
        }}
      >
        {t("deliver.export", { count })}
      </button>
    </div>
  );
}

/** Progression de l'export, puis bilan, dans la barre d'outils. */
export function ExportStatus() {
  const { t } = useTranslation();
  const running = useDeliver((s) => s.running);
  const progress = useDeliver((s) => s.progress);
  const finished = useDeliver((s) => s.finished);
  const st = useDeliver.getState();
  if (running && progress) {
    return (
      <div className="deliver-status">
        <progress value={progress.done} max={Math.max(1, progress.total)} />
        <span className="toolbar-status">
          {t("deliver.progress", { done: progress.done, total: progress.total })}
          {progress.current ? ` · ${progress.current}` : ""}
        </span>
        <button type="button" className="button button-small" onClick={() => void st.cancel()}>
          {t("deliver.stop")}
        </button>
      </div>
    );
  }
  if (!finished) return null;
  const p = finished.progress;
  const failures = p?.failures ?? [];
  const written = p ? p.done - failures.length : 0;
  return (
    <div className="deliver-status">
      <span
        className="toolbar-status"
        title={failures
          .map((f) => `${f.file} : ${t(`errors.${f.code}`, { defaultValue: f.detail })}`)
          .join("\n")}
      >
        {finished.cancelled
          ? t("deliver.cancelled", { count: written })
          : t("deliver.done", { count: written })}
        {failures.length > 0 && ` · ${t("deliver.failures", { count: failures.length })}`}
      </span>
      <IconButton
        icon={FolderOpen}
        label={t("deliver.reveal")}
        onClick={() =>
          void import("@tauri-apps/api/core").then(({ invoke }) =>
            invoke("export_reveal", { destination: finished.destination }),
          )
        }
      />
      <IconButton icon={X} label={t("deliver.dismiss")} onClick={st.dismiss} />
    </div>
  );
}
