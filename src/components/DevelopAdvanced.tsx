import { RotateCcw } from "lucide-react";
import { type PointerEvent, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  CURVE_CHANNELS,
  type CurveChannel,
  type CurvePoint,
  EFFECT_DEFAULTS,
  type EffectGroup,
  type EffectKey,
  HSL_KINDS,
  HUES,
  type HslKind,
  IDENTITY_CURVE,
  useDevelop,
} from "../store/develop";
import { curveAt } from "../ui/curve";
import { Slider } from "./DevelopPanels";
import { IconButton } from "./IconButton";
import { PanelSection } from "./PanelSection";

/** Boutons à choix unique (canal de la courbe, onglet TSL). */
function Segmented<T extends string>({
  label,
  options,
  value,
  onChange,
  text,
}: {
  label: string;
  options: readonly T[];
  value: T;
  onChange: (v: T) => void;
  text: (v: T) => string;
}) {
  return (
    <div className="develop-segmented" role="radiogroup" aria-label={label}>
      {options.map((o) => (
        <button
          key={o}
          type="button"
          role="radio"
          aria-checked={o === value}
          onClick={() => onChange(o)}
        >
          {text(o)}
        </button>
      ))}
    </div>
  );
}

const CURVE_STROKE: Record<CurveChannel, string> = {
  rgb: "var(--ad-text)",
  red: "rgb(235 80 80)",
  green: "rgb(80 200 110)",
  blue: "rgb(90 140 245)",
};

/** Distance (en unités 0 à 255) sous laquelle un clic saisit un point. */
const GRAB = 10;

/** Éditeur de courbe : clic pour ajouter un point, glisser pour le déplacer,
 * double-clic sur un point pour le retirer. */
function CurveEditor({ channel }: { channel: CurveChannel }) {
  const { t } = useTranslation();
  const stored = useDevelop((s) => s.settings?.curve?.[channel]);
  const histogram = useDevelop((s) => s.frame?.histogram ?? null);
  const disabled = useDevelop((s) => s.settings === null);
  const setCurve = useDevelop((s) => s.setCurve);
  const points = stored && stored.length >= 2 ? stored : IDENTITY_CURVE;
  const svg = useRef<SVGSVGElement>(null);
  const [drag, setDrag] = useState<number | null>(null);

  const toCurve = (e: PointerEvent<SVGSVGElement>): CurvePoint => {
    const r = e.currentTarget.getBoundingClientRect();
    const x = ((e.clientX - r.left) / r.width) * 255;
    const y = (1 - (e.clientY - r.top) / r.height) * 255;
    return [Math.min(255, Math.max(0, x)), Math.min(255, Math.max(0, y))];
  };

  /** Point `i` déplacé en `p`, sans dépasser ses voisins. */
  const moved = (list: CurvePoint[], i: number, p: CurvePoint): CurvePoint[] => {
    const lo = i === 0 ? 0 : (list[i - 1]?.[0] ?? 0) + 1;
    const hi = i === list.length - 1 ? 255 : (list[i + 1]?.[0] ?? 255) - 1;
    const next = list.slice();
    next[i] = [Math.min(hi, Math.max(lo, p[0])), p[1]];
    return next;
  };

  const onDown = (e: PointerEvent<SVGSVGElement>) => {
    if (disabled) return;
    const p = toCurve(e);
    const near = points.findIndex((q) => Math.hypot(q[0] - p[0], q[1] - p[1]) < GRAB);
    e.currentTarget.setPointerCapture(e.pointerId);
    if (near >= 0) {
      setDrag(near);
      return;
    }
    // Nouveau point sur la courbe, à l'abscisse du clic.
    const y = curveAt(points)(p[0]);
    const next = [...points, [p[0], y] as CurvePoint].sort((a, b) => a[0] - b[0]);
    if (next.length > 16) return;
    setDrag(next.findIndex((q) => q[0] === p[0]));
    setCurve(channel, next);
  };

  const onMove = (e: PointerEvent<SVGSVGElement>) => {
    if (drag === null) return;
    setCurve(channel, moved(points, drag, toCurve(e)));
  };

  const onDouble = (e: React.MouseEvent<SVGSVGElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    const x = ((e.clientX - r.left) / r.width) * 255;
    const y = (1 - (e.clientY - r.top) / r.height) * 255;
    const near = points.findIndex((q) => Math.hypot(q[0] - x, q[1] - y) < GRAB);
    // Les extrémités restent ; un point intérieur se retire.
    if (near > 0 && near < points.length - 1) {
      setCurve(
        channel,
        points.filter((_, i) => i !== near),
      );
    }
  };

  const f = curveAt(points);
  const path = Array.from({ length: 129 }, (_, i) => {
    const x = (i / 128) * 255;
    return `${i === 0 ? "M" : "L"}${x.toFixed(1)},${(255 - f(x)).toFixed(1)}`;
  }).join(" ");

  // Histogramme du canal (ou de la luminance) en fond, à la racine carrée.
  const offset = { rgb: 768, red: 0, green: 256, blue: 512 }[channel];
  let histogramPath = "";
  if (histogram) {
    let max = 1;
    for (let i = 1; i < 255; i++) max = Math.max(max, histogram[offset + i] ?? 0);
    histogramPath =
      "M0,255 " +
      Array.from({ length: 256 }, (_, i) => {
        const v = Math.sqrt(Math.min(1, (histogram[offset + i] ?? 0) / max));
        return `L${i},${(255 - v * 120).toFixed(1)}`;
      }).join(" ") +
      " L255,255 Z";
  }

  return (
    <svg
      ref={svg}
      className="develop-curve"
      viewBox="0 0 255 255"
      role="img"
      aria-label={t("develop.curve.editor")}
      data-dragging={drag !== null || undefined}
      onPointerDown={onDown}
      onPointerMove={onMove}
      onPointerUp={() => setDrag(null)}
      onPointerCancel={() => setDrag(null)}
      onDoubleClick={onDouble}
    >
      {histogramPath && <path className="develop-curve-histogram" d={histogramPath} />}
      {[64, 128, 191].map((v) => (
        <g key={v} className="develop-curve-grid">
          <line x1={v} y1={0} x2={v} y2={255} />
          <line x1={0} y1={v} x2={255} y2={v} />
        </g>
      ))}
      <line className="develop-curve-diagonal" x1={0} y1={255} x2={255} y2={0} />
      <path className="develop-curve-line" d={path} style={{ stroke: CURVE_STROKE[channel] }} />
      {points.map(([x, y], i) => (
        <circle
          key={i}
          className="develop-curve-point"
          cx={x}
          cy={255 - y}
          r={4}
          data-active={drag === i || undefined}
        />
      ))}
    </svg>
  );
}

export function CurveSection() {
  const { t } = useTranslation();
  const [channel, setChannel] = useState<CurveChannel>("rgb");
  const hasCurve = useDevelop((s) => !!s.settings?.curve?.[channel]);
  const setCurve = useDevelop((s) => s.setCurve);
  return (
    <PanelSection
      id="develop.curve"
      title={t("develop.curve.title")}
      aside={
        <span className="panel-section-aside develop-history-actions">
          <IconButton
            icon={RotateCcw}
            label={t("develop.curve.reset")}
            disabled={!hasCurve}
            onClick={() => setCurve(channel, IDENTITY_CURVE)}
          />
        </span>
      }
    >
      <Segmented
        label={t("develop.curve.channel")}
        options={CURVE_CHANNELS}
        value={channel}
        onChange={setChannel}
        text={(c) => t(`develop.curve.${c}`)}
      />
      <CurveEditor channel={channel} />
      <p className="panel-note">{t("develop.curve.hint")}</p>
    </PanelSection>
  );
}

const HUE_SWATCH: Record<(typeof HUES)[number], string> = {
  red: "rgb(230 60 60)",
  orange: "rgb(240 140 40)",
  yellow: "rgb(235 210 50)",
  green: "rgb(80 190 80)",
  aqua: "rgb(60 200 200)",
  blue: "rgb(60 110 230)",
  purple: "rgb(140 80 220)",
  magenta: "rgb(220 70 190)",
};

export function HslSection() {
  const { t, i18n } = useTranslation();
  const [kind, setKind] = useState<HslKind>("saturation");
  const values = useDevelop((s) => s.settings?.hsl?.[kind]);
  const disabled = useDevelop((s) => s.settings === null);
  const setHsl = useDevelop((s) => s.setHsl);
  const plain = new Intl.NumberFormat(i18n.language, { signDisplay: "exceptZero" });
  return (
    <PanelSection id="develop.hsl" title={t("develop.hsl.title")}>
      <Segmented
        label={t("develop.hsl.title")}
        options={HSL_KINDS}
        value={kind}
        onChange={setKind}
        text={(k) => t(`develop.hsl.${k}`)}
      />
      {HUES.map((hue) => (
        <div key={hue} className="develop-hue-row" style={{ "--hue": HUE_SWATCH[hue] } as never}>
          <Slider
            label={t(`develop.hues.${hue}`)}
            display={plain.format(values?.[hue] ?? 0)}
            value={values?.[hue] ?? 0}
            min={-100}
            max={100}
            step={1}
            disabled={disabled}
            onChange={(v) => setHsl(kind, hue, v)}
            onReset={() => setHsl(kind, hue, 0)}
          />
        </div>
      ))}
    </PanelSection>
  );
}

function EffectSliders<G extends EffectGroup>({ group }: { group: G }) {
  const { t, i18n } = useTranslation();
  const section = useDevelop((s) => s.settings?.effects?.[group]) as
    Partial<Record<string, number>> | undefined;
  const disabled = useDevelop((s) => s.settings === null);
  const setEffect = useDevelop((s) => s.setEffect);
  const defaults = EFFECT_DEFAULTS[group] as Record<string, number>;
  const signed = new Intl.NumberFormat(i18n.language, { signDisplay: "exceptZero" });
  const plain = new Intl.NumberFormat(i18n.language);
  return (
    <>
      <h3 className="develop-subheading">{t(`develop.effects.${group}`)}</h3>
      {Object.keys(defaults).map((key) => {
        const value = section?.[key] ?? defaults[key] ?? 0;
        const min = group === "vignette" && key === "amount" ? -100 : 0;
        return (
          <Slider
            key={key}
            label={t(`develop.effects.${group}_${key}`)}
            display={(min < 0 ? signed : plain).format(value)}
            value={value}
            min={min}
            max={100}
            step={1}
            disabled={disabled}
            onChange={(v) => setEffect(group, key as EffectKey<G>, v)}
            onReset={() => setEffect(group, key as EffectKey<G>, defaults[key] ?? 0)}
          />
        );
      })}
    </>
  );
}

export function EffectsSection() {
  const { t } = useTranslation();
  return (
    <PanelSection id="develop.effects" title={t("develop.effects.title")}>
      <EffectSliders group="vignette" />
      <EffectSliders group="grain" />
    </PanelSection>
  );
}
