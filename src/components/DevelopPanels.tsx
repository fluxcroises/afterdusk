import { Pipette } from "lucide-react";
import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import {
  COLOR_KEYS,
  type ClippingWarnings,
  EXPOSURE_RANGE,
  LIGHT_KEYS,
  type LightKey,
  SLIDER_RANGE,
  TINT_RANGE,
  type WhiteBalance,
  positionFromTemperature,
  temperatureFromPosition,
  useDevelop,
  whitePoint,
} from "../store/develop";
import { useLibrary } from "../store/library";
import { formatAperture, formatFocal, formatIso, formatShutter } from "../ui/format";
import { IconButton } from "./IconButton";
import { PanelSection } from "./PanelSection";

interface SliderProps {
  label: string;
  /** Valeur affichée à droite du libellé. */
  display: string;
  value: number;
  min: number;
  max: number;
  step: number;
  disabled?: boolean;
  onChange: (value: number) => void;
  /** Double-clic : retour à la valeur neutre. */
  onReset: () => void;
  /** Maj + double-clic : valeur automatique, pour les curseurs qui en ont une. */
  onAuto?: () => void;
}

export function Slider({
  label,
  display,
  value,
  min,
  max,
  step,
  disabled,
  onChange,
  onReset,
  onAuto,
}: SliderProps) {
  const { t } = useTranslation();
  return (
    <label className="develop-slider">
      <span className="develop-slider-head">
        <span>{label}</span>
        <output>{display}</output>
      </span>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        disabled={disabled}
        title={t(onAuto ? "develop.resetAutoHint" : "develop.resetHint")}
        onChange={(e) => onChange(Number(e.currentTarget.value))}
        onDoubleClick={(e) => (e.shiftKey && onAuto ? onAuto() : onReset())}
      />
    </label>
  );
}

function useNumberFormat(fractionDigits: number) {
  const { i18n } = useTranslation();
  return new Intl.NumberFormat(i18n.language, {
    signDisplay: "exceptZero",
    minimumFractionDigits: fractionDigits,
    maximumFractionDigits: fractionDigits,
  });
}

/** Part des pixels au-delà de laquelle l'indicateur d'écrêtage s'allume. */
const CLIP_SHARE = 1e-4;

function clippedShare(histogram: Uint32Array, bin: number) {
  let total = 0;
  for (let i = 0; i < 256; i++) total += histogram[i] ?? 0;
  if (total === 0) return 0;
  const clipped = Math.max(
    histogram[bin] ?? 0,
    histogram[256 + bin] ?? 0,
    histogram[512 + bin] ?? 0,
  );
  return clipped / total;
}

const CHANNEL_COLORS = [
  "rgb(235 80 80 / 0.75)",
  "rgb(80 200 110 / 0.75)",
  "rgb(90 140 245 / 0.75)",
];

/** Histogramme de l'image rendue : rouge, vert et bleu superposés. */
function HistogramCanvas({ histogram }: { histogram: Uint32Array | null }) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const canvas = ref.current;
    const ctx = canvas?.getContext("2d");
    if (!canvas || !ctx) return;
    const { width, height } = canvas;
    ctx.clearRect(0, 0, width, height);
    if (!histogram) return;
    // Échelle sur les cases intérieures : un pic d'écrêtage n'écrase pas le reste.
    let max = 1;
    for (let c = 0; c < 3; c++)
      for (let i = 1; i < 255; i++) max = Math.max(max, histogram[c * 256 + i] ?? 0);
    ctx.globalCompositeOperation = "lighter";
    for (let c = 0; c < 3; c++) {
      ctx.beginPath();
      ctx.moveTo(0, height);
      for (let i = 0; i < 256; i++) {
        const v = Math.min(1, (histogram[c * 256 + i] ?? 0) / max);
        ctx.lineTo((i / 255) * width, height - v * height);
      }
      ctx.lineTo(width, height);
      ctx.closePath();
      ctx.fillStyle = CHANNEL_COLORS[c] ?? "white";
      ctx.fill();
    }
    ctx.globalCompositeOperation = "source-over";
  }, [histogram]);
  return <canvas ref={ref} className="develop-histogram-canvas" width={256} height={96} />;
}

function ClipButton({ which, lit }: { which: keyof ClippingWarnings; lit: boolean }) {
  const { t } = useTranslation();
  const active = useDevelop((s) => s.clipping[which]);
  const toggle = useDevelop((s) => s.toggleClipping);
  return (
    <button
      type="button"
      className={`develop-clip develop-clip-${which}`}
      aria-pressed={active}
      data-lit={lit || undefined}
      title={t(`develop.clip.${which}`)}
      aria-label={t(`develop.clip.${which}`)}
      onClick={() => toggle(which)}
    />
  );
}

export function HistogramSection() {
  const { t, i18n } = useTranslation();
  const histogram = useDevelop((s) => s.frame?.histogram ?? null);
  const d = useLibrary((s) => s.details);
  const lng = i18n.language;
  const shadows = histogram ? clippedShare(histogram, 0) > CLIP_SHARE : false;
  const highlights = histogram ? clippedShare(histogram, 255) > CLIP_SHARE : false;
  return (
    <PanelSection id="develop.histogram" title={t("info.histogram")}>
      <div className="histogram develop-histogram">
        <HistogramCanvas histogram={histogram} />
        <ClipButton which="shadows" lit={shadows} />
        <ClipButton which="highlights" lit={highlights} />
      </div>
      <div className="histogram-exif">
        <span>{formatIso(d?.iso ?? null) ?? "ISO —"}</span>
        <span>{formatFocal(d?.focalLength ?? null, lng) ?? "— mm"}</span>
        <span>{formatAperture(d?.aperture ?? null, lng) ?? "f / —"}</span>
        <span>{formatShutter(d?.shutter ?? null) ?? "— s"}</span>
      </div>
    </PanelSection>
  );
}

export function LightSection() {
  const { t } = useTranslation();
  const settings = useDevelop((s) => s.settings);
  const setLight = useDevelop((s) => s.setLight);
  const autoLight = useDevelop((s) => s.autoLight);
  const busy = useDevelop((s) => s.autoBusy);
  const ev = useNumberFormat(2);
  const plain = useNumberFormat(0);
  const value = (key: LightKey) => settings?.light?.[key] ?? 0;
  return (
    <PanelSection
      id="develop.light"
      title={t("develop.light")}
      aside={
        <span className="panel-section-aside">
          <button
            type="button"
            className="button button-small develop-auto"
            disabled={!settings || busy}
            title={t("develop.autoHint")}
            onClick={() => void autoLight()}
          >
            {t("develop.auto")}
          </button>
        </span>
      }
    >
      {LIGHT_KEYS.map((key) => (
        <Slider
          key={key}
          label={t(`develop.${key}`)}
          display={
            key === "exposure"
              ? t("develop.ev", { value: ev.format(value(key)) })
              : plain.format(value(key))
          }
          value={value(key)}
          {...(key === "exposure" ? EXPOSURE_RANGE : SLIDER_RANGE)}
          disabled={!settings}
          onChange={(v) => setLight(key, v)}
          onReset={() => setLight(key, 0)}
          // La luminosité n'a pas d'auto : c'est un choix d'interprétation.
          onAuto={key === "brightness" ? undefined : () => void autoLight(key)}
        />
      ))}
    </PanelSection>
  );
}

function wbChoice(wb: WhiteBalance | undefined): string {
  if (!wb || wb.mode === "asShot") return "asShot";
  return wb.mode === "preset" ? wb.preset : "custom";
}

export function ColorSection() {
  const { t, i18n } = useTranslation();
  const settings = useDevelop((s) => s.settings);
  const asShot = useDevelop((s) => s.asShot);
  const presets = useDevelop((s) => s.presets);
  const picking = useDevelop((s) => s.picking);
  const { setWhiteBalance, setTemperature, setTint, setPicking, setColor } = useDevelop.getState();
  const plain = useNumberFormat(0);
  const kelvin = new Intl.NumberFormat(i18n.language, { maximumFractionDigits: 0 });
  const wb = settings?.color?.whiteBalance;
  const point = asShot ? whitePoint(wb, asShot, presets) : null;
  const disabled = !settings || !point;

  const choose = (choice: string) => {
    if (choice === "auto") void useDevelop.getState().autoWhite();
    else if (choice === "asShot") setWhiteBalance({ mode: "asShot" });
    else if (choice === "custom" && point) setWhiteBalance({ mode: "custom", ...point });
    else setWhiteBalance({ mode: "preset", preset: choice });
  };

  return (
    <PanelSection id="develop.color" title={t("develop.color")}>
      <div className="develop-wb">
        <label className="develop-wb-select">
          <span>{t("develop.whiteBalance")}</span>
          <select
            value={wbChoice(wb)}
            disabled={disabled}
            onChange={(e) => choose(e.currentTarget.value)}
          >
            <option value="asShot">{t("develop.wb.asShot")}</option>
            <option value="auto">{t("develop.wb.auto")}</option>
            {presets.map((p) => (
              <option key={p.name} value={p.name}>
                {t(`develop.wb.${p.name}`)}
              </option>
            ))}
            <option value="custom">{t("develop.wb.custom")}</option>
          </select>
        </label>
        <IconButton
          icon={Pipette}
          label={t("develop.pick")}
          pressed={picking}
          disabled={disabled}
          onClick={() => setPicking(!picking)}
        />
      </div>
      <Slider
        label={t("develop.temperature")}
        display={point ? t("develop.kelvin", { value: kelvin.format(point.temperature) }) : "—"}
        value={point ? Math.round(positionFromTemperature(point.temperature) * 1000) : 500}
        min={0}
        max={1000}
        step={1}
        disabled={disabled}
        onChange={(v) => setTemperature(temperatureFromPosition(v / 1000))}
        onReset={() => setWhiteBalance({ mode: "asShot" })}
        onAuto={() => void useDevelop.getState().autoWhite()}
      />
      <Slider
        label={t("develop.tint")}
        display={point ? plain.format(point.tint) : "—"}
        value={point?.tint ?? 0}
        {...TINT_RANGE}
        disabled={disabled}
        onChange={setTint}
        onReset={() => setWhiteBalance({ mode: "asShot" })}
        onAuto={() => void useDevelop.getState().autoWhite()}
      />
      {COLOR_KEYS.map((key) => (
        <Slider
          key={key}
          label={t(`develop.${key}`)}
          display={plain.format(settings?.color?.[key] ?? 0)}
          value={settings?.color?.[key] ?? 0}
          {...SLIDER_RANGE}
          disabled={!settings}
          onChange={(v) => setColor(key, v)}
          onReset={() => setColor(key, 0)}
        />
      ))}
    </PanelSection>
  );
}
