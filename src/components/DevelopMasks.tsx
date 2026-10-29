import { Eraser, Eye, EyeOff, Plus } from "lucide-react";
import {
  type MouseEvent,
  type PointerEvent,
  type WheelEvent,
  useEffect,
  useRef,
  useState,
} from "react";
import { useTranslation } from "react-i18next";
import { openDevelopMenu, openMaskMenu } from "../contextMenu";
import {
  BRUSH_RADIUS,
  LOCAL_EXPOSURE_RANGE,
  LOCAL_KEYS,
  type LocalKey,
  MASKS_MAX,
  type Mask,
  type MaskShape,
  SLIDER_RANGE,
  useDevelop,
} from "../store/develop";
import { outputSize, outputToSource, sourceToOutput } from "../ui/geometry";
import {
  MASK_FEATHER_DEFAULT,
  MASK_SIZE_MIN,
  maskName,
  maskViewPixels,
  movedShape,
  normalizeAngle,
} from "../ui/masks";
import type { Box } from "./DevelopCrop";
import type { View } from "./DevelopSpots";
import { IconButton } from "./IconButton";
import { Slider } from "./DevelopPanels";
import { PanelSection } from "./PanelSection";

/** Distance (pixels de l'écran) sous laquelle un appui est un clic. */
const CLICK_SLOP = 4;
/** Rayon de prise d'une poignée, en pixels de l'écran. */
const GRAB = 8;
/** Écart de la poignée de rotation au bord de l'ellipse. */
const ROTATE_GAP = 22;
/** Côté long du calque du masque, en pixels (assez fin, vite calculé). */
const VIEW_SIDE = 480;

type Pt = [number, number];

type Handle = "move" | "from" | "to" | "u+" | "u-" | "v+" | "v-" | "rotate";

type Target = { index: number; handle: Handle } | { create: MaskShape["kind"] };

/** Tracé de pinceau en cours : points en fractions de la source. */
interface Painting {
  points: Pt[];
  erase: boolean;
}

interface Press {
  /** Position de l'écran au dernier déplacement de la région (zoom). */
  x: number;
  y: number;
  /** Point de l'appui, dans le calque. */
  start: Pt;
  moved: boolean;
  target: Target | null;
}

const dist = (a: Pt, b: Pt) => Math.hypot(a[0] - b[0], a[1] - b[1]);

/** Poignées d'un masque à l'écran, et ce qu'il faut pour le dessiner. */
function screenShape(shape: MaskShape, toScreen: (p: Pt) => Pt, sw: number, sh: number) {
  // Pinceau : ni poignée ni contour, ses tracés le montrent (V0.7).
  if (shape.kind === "brush") {
    return { kind: "brush" as const, handles: {} as Partial<Record<Handle, Pt>> };
  }
  if (shape.kind === "linear") {
    const from = toScreen(shape.from);
    const to = toScreen(shape.to);
    const mid: Pt = [(from[0] + to[0]) / 2, (from[1] + to[1]) / 2];
    return { kind: "linear" as const, from, to, mid, handles: { move: mid, from, to } };
  }
  const a = ((shape.angle ?? 0) * Math.PI) / 180;
  const [sin, cos] = [Math.sin(a), Math.cos(a)];
  const [rx, ry] = [shape.radius[0] * sw, shape.radius[1] * sh];
  // Point à `len` pixels de la source du centre, dans la direction `d`.
  const along = (len: number, d: Pt) =>
    toScreen([shape.center[0] + (d[0] * len) / sw, shape.center[1] + (d[1] * len) / sh]);
  const center = toScreen(shape.center);
  const u = along(rx, [cos, sin]);
  const v = along(ry, [-sin, cos]);
  const vm = along(ry, [sin, -cos]);
  const up = Math.max(dist(vm, center), 1e-9);
  const rotate: Pt = [
    vm[0] + ((vm[0] - center[0]) / up) * ROTATE_GAP,
    vm[1] + ((vm[1] - center[1]) / up) * ROTATE_GAP,
  ];
  return {
    kind: "radial" as const,
    center,
    rx: dist(u, center),
    ry: dist(v, center),
    rotation: (Math.atan2(u[1] - center[1], u[0] - center[0]) * 180) / Math.PI,
    inner: 1 - (shape.feather ?? MASK_FEATHER_DEFAULT) / 100,
    vm,
    handles: {
      move: center,
      "u+": u,
      "u-": along(rx, [-cos, -sin]),
      "v+": v,
      "v-": vm,
      rotate,
    } as Partial<Record<Handle, Pt>>,
  };
}

/**
 * Masques posés sur l'image. Outil armé (« + Linéaire », « + Radial ») : un
 * glissé trace le masque. Sinon, les poignées du masque choisi le déplacent,
 * le redimensionnent ou le font tourner (Maj : cercle, ou par pas de 15°), et
 * la pastille d'un autre masque le choisit. Pinceau choisi : un glissé ailleurs
 * peint, Alt maintenu gomme ; sur un dégradé, Alt ou la gomme du panneau font
 * gommer. Au zoom, un glissé ailleurs déplace la région. Formes et tracés ne
 * sont enregistrés qu'au relâchement.
 */
export function MaskOverlay({ box, view }: { box: Box; view: View }) {
  const { t } = useTranslation();
  const masks = useDevelop((s) => s.settings?.masks) ?? [];
  const geometry = useDevelop((s) => s.settings?.geometry) ?? {};
  const source = useDevelop((s) => s.sourceSize);
  const selected = useDevelop((s) => s.selectedMask);
  const armed = useDevelop((s) => s.maskDraft);
  const showMask = useDevelop((s) => s.showMask);
  const zoomed = useDevelop((s) => s.zoom !== null);
  const brush = useDevelop((s) => s.maskBrush);
  const erasing = useDevelop((s) => s.maskErasing);
  const [press, setPress] = useState<Press | null>(null);
  const [draft, setDraft] = useState<MaskShape | null>(null);
  const [painting, setPainting] = useState<Painting | null>(null);
  const [hover, setHover] = useState<{ at: Pt; alt: boolean } | null>(null);
  const viewRef = useRef<HTMLCanvasElement>(null);

  const sw = source?.width ?? 1;
  const sh = source?.height ?? 1;
  const toScreen = (p: Pt): Pt => {
    const [fx, fy] = sourceToOutput(p[0], p[1], geometry, sw, sh);
    return [
      ((fx - view.x0) / (view.x1 - view.x0)) * box.width,
      ((fy - view.y0) / (view.y1 - view.y0)) * box.height,
    ];
  };
  const toSource = (p: Pt): Pt =>
    outputToSource(
      view.x0 + (p[0] / box.width) * (view.x1 - view.x0),
      view.y0 + (p[1] / box.height) * (view.y1 - view.y0),
      geometry,
      sw,
      sh,
    );

  // Masque montré en rouge : le choisi, ou celui en cours de tracé.
  const editing = press?.target && "index" in press.target ? press.target.index : null;
  const chosen = selected === null ? undefined : masks[selected];
  const viewed: Mask | undefined =
    draft && press?.target && "create" in press.target
      ? { shape: draft }
      : chosen && draft && editing === selected
        ? { ...chosen, shape: draft }
        : chosen;

  // Pinceau : pixels de l'écran par pixel de la source, rayon à l'écran.
  const [outW] = outputSize(geometry, sw, sh);
  const scale = box.width / Math.max(1e-9, (view.x1 - view.x0) * outW);
  const brushRadius = brush.radius * Math.max(sw, sh) * scale;
  const isBrush = chosen?.shape.kind === "brush";
  /** Un glissé hors des poignées peint ou gomme le masque choisi. */
  const paints = (alt: boolean) => !armed && chosen !== undefined && (isBrush || erasing || alt);
  const viewKey = showMask && viewed ? JSON.stringify([viewed, geometry, view, box, sw, sh]) : "";

  useEffect(() => {
    const canvas = viewRef.current;
    const context = canvas?.getContext("2d");
    if (!canvas || !context || !viewKey || !viewed) return;
    const k = Math.min(1, VIEW_SIDE / Math.max(box.width, box.height));
    const w = Math.max(1, Math.round(box.width * k));
    const h = Math.max(1, Math.round(box.height * k));
    canvas.width = w;
    canvas.height = h;
    const pixels = maskViewPixels(
      w,
      h,
      (fx, fy) => toSource([fx * box.width, fy * box.height]),
      viewed,
      sw,
      sh,
    );
    context.putImageData(new ImageData(pixels, w, h), 0, 0);
    // viewKey résume tout ce dont le calque dépend.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [viewKey]);

  if (!source) return null;

  const local = (e: MouseEvent<HTMLDivElement>): Pt => {
    const rect = e.currentTarget.getBoundingClientRect();
    return [e.clientX - rect.left, e.clientY - rect.top];
  };

  /** Poignée du masque choisi, ou pastille d'un masque, sous le pointeur. */
  const hitAt = (p: Pt): { index: number; handle: Handle } | null => {
    if (chosen && selected !== null) {
      const { handles } = screenShape(chosen.shape, toScreen, sw, sh);
      for (const [handle, at] of Object.entries(handles) as [Handle, Pt][]) {
        if (dist(p, at) <= GRAB) return { index: selected, handle };
      }
    }
    for (let i = masks.length - 1; i >= 0; i--) {
      const m = masks[i];
      const pin = m && screenShape(m.shape, toScreen, sw, sh).handles.move;
      if (pin && dist(p, pin) <= GRAB) {
        return { index: i, handle: "move" };
      }
    }
    return null;
  };

  /** Forme tirée par le glissé, du point d'appui `start` au point `now`. */
  const shapeFor = (target: Target, start: Pt, now: Pt, shift: boolean): MaskShape | null => {
    const s0 = toSource(start);
    const s1 = toSource(now);
    if ("create" in target) {
      if (target.create === "linear") return { kind: "linear", from: s0, to: s1 };
      let rx = Math.abs(s1[0] - s0[0]) * sw;
      let ry = Math.abs(s1[1] - s0[1]) * sh;
      if (shift) rx = ry = Math.hypot(rx, ry);
      return {
        kind: "radial",
        center: s0,
        radius: [Math.max(MASK_SIZE_MIN, rx / sw), Math.max(MASK_SIZE_MIN, ry / sh)],
      };
    }
    const shape = masks[target.index]?.shape;
    if (!shape) return null;
    if (target.handle === "move") return movedShape(shape, s1[0] - s0[0], s1[1] - s0[1]);
    if (shape.kind === "brush") return null;
    if (shape.kind === "linear") {
      return target.handle === "from" ? { ...shape, from: s1 } : { ...shape, to: s1 };
    }
    // Vecteur du centre au pointeur, en pixels de la source.
    const w: Pt = [(s1[0] - shape.center[0]) * sw, (s1[1] - shape.center[1]) * sh];
    const a = ((shape.angle ?? 0) * Math.PI) / 180;
    const [sin, cos] = [Math.sin(a), Math.cos(a)];
    if (target.handle === "rotate") {
      const angle = (Math.atan2(w[0], -w[1]) * 180) / Math.PI;
      return { ...shape, angle: normalizeAngle(shift ? Math.round(angle / 15) * 15 : angle) };
    }
    const alongU = target.handle === "u+" || target.handle === "u-";
    const len = Math.abs(alongU ? w[0] * cos + w[1] * sin : -w[0] * sin + w[1] * cos);
    const [rx, ry] = shift
      ? [len / sw, len / sh]
      : alongU
        ? [len / sw, shape.radius[1]]
        : [shape.radius[0], len / sh];
    return { ...shape, radius: [Math.max(MASK_SIZE_MIN, rx), Math.max(MASK_SIZE_MIN, ry)] };
  };

  const down = (e: PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    const start = local(e);
    const hit = hitAt(start);
    if (!hit && paints(e.altKey)) {
      // Pinceau : ajout, sauf Alt ou gomme choisie ; dégradé : toujours gomme.
      setPainting({ points: [toSource(start)], erase: !isBrush || erasing || e.altKey });
      return;
    }
    const target: Target | null = armed ? { create: armed } : hit;
    setPress({ x: e.clientX, y: e.clientY, start, moved: false, target });
  };

  const move = (e: PointerEvent<HTMLDivElement>) => {
    const at = local(e);
    setHover({ at, alt: e.altKey });
    if (painting) {
      setPainting({ ...painting, points: [...painting.points, toSource(at)] });
      return;
    }
    if (!press) return;
    const dx = e.clientX - press.x;
    const dy = e.clientY - press.y;
    if (!press.moved && Math.hypot(dx, dy) < CLICK_SLOP) return;
    if (press.target) {
      setPress({ ...press, moved: true });
      setDraft(shapeFor(press.target, press.start, local(e), e.shiftKey));
    } else if (zoomed) {
      const dpr = window.devicePixelRatio || 1;
      setPress({ ...press, x: e.clientX, y: e.clientY, moved: true });
      useDevelop.getState().panZoom(dx * dpr, dy * dpr);
    } else {
      setPress({ ...press, moved: true });
    }
  };

  const up = () => {
    if (painting) {
      const stroke = painting;
      setPainting(null);
      if (selected !== null) {
        void useDevelop.getState().addStroke(selected, stroke.points, stroke.erase);
      }
      return;
    }
    const p = press;
    const shape = draft;
    setPress(null);
    setDraft(null);
    if (!p) return;
    const state = useDevelop.getState();
    if (p.target && "create" in p.target) {
      if (p.moved && shape) void state.addMask(shape);
      return;
    }
    if (p.target) {
      if (p.moved && shape) void state.setMaskShape(p.target.index, shape);
      else state.selectMask(p.target.index);
      return;
    }
    if (!p.moved) state.selectMask(null);
  };

  const drawn = masks.map((m, i) => (draft && editing === i ? { ...m, shape: draft } : m));
  const creating = draft && press?.target && "create" in press.target ? draft : null;
  const reach = 2 * (box.width + box.height);

  const outline = (shape: MaskShape, key: string, active: boolean, hidden = false) => {
    const s = screenShape(shape, toScreen, sw, sh);
    if (!active) {
      return (
        <circle
          key={key}
          className="develop-mask-pin"
          data-hidden={hidden || undefined}
          cx={s.handles.move?.[0]}
          cy={s.handles.move?.[1]}
          r={5}
        />
      );
    }
    if (s.kind === "brush") return null;
    if (s.kind === "linear") {
      const d: Pt = [s.to[0] - s.from[0], s.to[1] - s.from[1]];
      const len = Math.max(Math.hypot(d[0], d[1]), 1e-9);
      const n: Pt = [(-d[1] / len) * reach, (d[0] / len) * reach];
      const line = (p: Pt, className: string) => (
        <line
          className={className}
          x1={p[0] - n[0]}
          y1={p[1] - n[1]}
          x2={p[0] + n[0]}
          y2={p[1] + n[1]}
        />
      );
      return (
        <g key={key}>
          {line(s.from, "develop-mask-line")}
          {line(s.mid, "develop-mask-line develop-mask-line-mid")}
          {line(s.to, "develop-mask-line")}
          <circle className="develop-mask-handle" cx={s.from[0]} cy={s.from[1]} r={4} />
          <circle className="develop-mask-handle" cx={s.to[0]} cy={s.to[1]} r={4} />
          <circle className="develop-mask-pin" data-selected cx={s.mid[0]} cy={s.mid[1]} r={6} />
        </g>
      );
    }
    const transform = `rotate(${s.rotation} ${s.center[0]} ${s.center[1]})`;
    return (
      <g key={key}>
        <ellipse
          className="develop-mask-line"
          cx={s.center[0]}
          cy={s.center[1]}
          rx={s.rx}
          ry={s.ry}
          transform={transform}
        />
        {s.inner > 0.01 && s.inner < 0.99 && (
          <ellipse
            className="develop-mask-line develop-mask-line-mid"
            cx={s.center[0]}
            cy={s.center[1]}
            rx={s.rx * s.inner}
            ry={s.ry * s.inner}
            transform={transform}
          />
        )}
        <line
          className="develop-mask-line develop-mask-line-mid"
          x1={s.vm[0]}
          y1={s.vm[1]}
          x2={s.handles.rotate?.[0]}
          y2={s.handles.rotate?.[1]}
        />
        {(["u+", "u-", "v+", "v-", "rotate"] as const).map((h) => (
          <circle
            key={h}
            className="develop-mask-handle"
            cx={s.handles[h]?.[0]}
            cy={s.handles[h]?.[1]}
            r={4}
          />
        ))}
        <circle
          className="develop-mask-pin"
          data-selected
          cx={s.center[0]}
          cy={s.center[1]}
          r={6}
        />
      </g>
    );
  };

  return (
    <div
      className="develop-masks"
      role="group"
      aria-label={t("develop.masks.title")}
      style={{ left: box.left, top: box.top, width: box.width, height: box.height }}
      data-armed={armed ?? undefined}
      data-painting={(hover && paints(hover.alt)) || undefined}
      data-dragging={(press?.moved ?? false) || undefined}
      onPointerDown={down}
      onPointerMove={move}
      onPointerUp={up}
      onPointerCancel={() => {
        setPress(null);
        setDraft(null);
        setPainting(null);
      }}
      onPointerLeave={() => setHover(null)}
      onWheel={(e: WheelEvent<HTMLDivElement>) => {
        // Molette : taille du pinceau, quand un glissé peindrait.
        if (!paints(e.altKey)) return;
        const factor = e.deltaY < 0 ? 1.15 : 1 / 1.15;
        useDevelop.getState().setMaskBrush({ radius: brush.radius * factor });
      }}
      onContextMenu={(e) => {
        e.preventDefault();
        const hit = hitAt(local(e));
        void (hit ? openMaskMenu(hit.index) : openDevelopMenu());
      }}
    >
      {showMask && viewed && <canvas ref={viewRef} className="develop-mask-view" />}
      <svg width={box.width} height={box.height} aria-hidden="true">
        {drawn.map((m, i) => outline(m.shape, String(i), i === selected, m.hidden))}
        {creating && outline(creating, "draft", true)}
        {painting && (
          <polyline
            className="develop-mask-stroke"
            data-erase={painting.erase || undefined}
            points={painting.points.map((p) => toScreen(p).join(",")).join(" ")}
            strokeWidth={Math.max(1, brushRadius * 2)}
          />
        )}
        {hover && !press && paints(hover.alt) && (
          <circle
            className="develop-mask-brush"
            data-erase={!isBrush || erasing || hover.alt || undefined}
            cx={hover.at[0]}
            cy={hover.at[1]}
            r={brushRadius}
          />
        )}
      </svg>
    </div>
  );
}

/** Curseurs d'un masque : plages et affichage, comme ceux de la photo. */
function useLocalFormat() {
  const { t, i18n } = useTranslation();
  const fmt = (digits: number) =>
    new Intl.NumberFormat(i18n.language, {
      signDisplay: "exceptZero",
      minimumFractionDigits: digits,
      maximumFractionDigits: digits,
    });
  const ev = fmt(2);
  const plain = fmt(0);
  return (key: LocalKey, v: number) =>
    key === "exposure" ? t("develop.ev", { value: ev.format(v) }) : plain.format(v);
}

/** Panneau de l'outil : création, liste des masques, réglages du choisi. */
export function MasksSection() {
  const { t } = useTranslation();
  const masks = useDevelop((s) => s.settings?.masks) ?? [];
  const selected = useDevelop((s) => s.selectedMask);
  const armed = useDevelop((s) => s.maskDraft);
  const showMask = useDevelop((s) => s.showMask);
  const format = useLocalFormat();
  const st = useDevelop.getState();
  const full = masks.length >= MASKS_MAX;
  const chosen = selected === null ? undefined : masks[selected];
  const kinds: readonly MaskShape["kind"][] = ["linear", "radial", "brush"];
  const brush = useDevelop((s) => s.maskBrush);
  const erasing = useDevelop((s) => s.maskErasing);

  return (
    <PanelSection
      id="develop.masks"
      title={t("develop.masks.title")}
      aside={<span className="panel-section-aside">{masks.length > 0 ? masks.length : ""}</span>}
    >
      <div className="develop-mask-add">
        {kinds.map((kind) => (
          <button
            key={kind}
            type="button"
            className="button button-small"
            aria-pressed={kind === "brush" ? undefined : armed === kind}
            disabled={full}
            title={t(full ? "develop.masks.limit" : `develop.masks.draw.${kind}`, {
              count: MASKS_MAX,
            })}
            onClick={() =>
              kind === "brush"
                ? void st.addMask({ kind: "brush" })
                : st.armMask(armed === kind ? null : kind)
            }
          >
            <Plus aria-hidden="true" strokeWidth={1.75} />
            {t(`develop.masks.kinds.${kind}`)}
          </button>
        ))}
      </div>
      {(armed || masks.length === 0) && (
        <p className="panel-note">
          {t(armed ? `develop.masks.draw.${armed}` : "develop.masks.hint")}
        </p>
      )}
      {masks.length > 0 && (
        <ul className="develop-mask-list">
          {masks.map((m, i) => (
            <li key={i} className="develop-mask-row" data-hidden={m.hidden || undefined}>
              <button
                type="button"
                className="list-row"
                aria-current={i === selected || undefined}
                onClick={() => st.selectMask(i === selected ? null : i)}
                onContextMenu={(e) => {
                  e.preventDefault();
                  void openMaskMenu(i);
                }}
              >
                <span className="list-row-label">{maskName(t, masks, i)}</span>
              </button>
              <IconButton
                icon={m.hidden ? EyeOff : Eye}
                label={t(m.hidden ? "develop.masks.show" : "develop.masks.hide")}
                pressed={m.hidden ?? false}
                onClick={() => void st.toggleMask(i, "hidden")}
              />
            </li>
          ))}
        </ul>
      )}
      {masks.length > 0 && (
        <label className="check-row">
          <input
            type="checkbox"
            checked={showMask}
            onChange={(e) => st.setShowMask(e.currentTarget.checked)}
          />
          {t("develop.masks.view")}
        </label>
      )}
      {chosen && selected !== null && (
        <div className="develop-mask-detail">
          <h3 className="develop-subheading">{maskName(t, masks, selected)}</h3>
          <label className="check-row">
            <input
              type="checkbox"
              checked={chosen.invert ?? false}
              onChange={() => void st.toggleMask(selected, "invert")}
            />
            {t("develop.masks.invert")}
          </label>
          {chosen.shape.kind === "radial" && (
            <Slider
              label={t("develop.masks.feather")}
              display={String(chosen.shape.feather ?? MASK_FEATHER_DEFAULT)}
              value={chosen.shape.feather ?? MASK_FEATHER_DEFAULT}
              min={0}
              max={100}
              step={1}
              onChange={(v) => st.setMaskFeather(selected, v)}
              onReset={() => st.setMaskFeather(selected, MASK_FEATHER_DEFAULT)}
            />
          )}
          <h3 className="develop-subheading">
            {t(
              chosen.shape.kind === "brush"
                ? "develop.masks.brush.title"
                : "develop.masks.brush.eraser",
            )}
          </h3>
          <div className="develop-crop-actions">
            <button
              type="button"
              className="button button-small"
              aria-pressed={erasing}
              onClick={() => st.setMaskErasing(!erasing)}
            >
              <Eraser aria-hidden="true" strokeWidth={1.75} />
              {t("develop.masks.brush.erase")}
            </button>
            <span className="toolbar-spacer" />
            {(chosen.strokes?.length ?? 0) > 0 && (
              <button
                type="button"
                className="button button-small"
                onClick={() => void st.clearStrokes(selected)}
              >
                {t("develop.masks.brush.clear")}
              </button>
            )}
          </div>
          <Slider
            label={t("develop.masks.brush.size")}
            display={String(Math.round(brush.radius * 1000))}
            value={Math.round(brush.radius * 1000)}
            min={Math.round(BRUSH_RADIUS.min * 1000)}
            max={200}
            step={1}
            onChange={(v) => st.setMaskBrush({ radius: v / 1000 })}
            onReset={() => st.setMaskBrush({ radius: 0.03 })}
          />
          <Slider
            label={t("develop.masks.feather")}
            display={String(brush.feather)}
            value={brush.feather}
            min={0}
            max={100}
            step={1}
            onChange={(v) => st.setMaskBrush({ feather: v })}
            onReset={() => st.setMaskBrush({ feather: MASK_FEATHER_DEFAULT })}
          />
          <Slider
            label={t("develop.masks.brush.flow")}
            display={`${brush.flow} %`}
            value={brush.flow}
            min={1}
            max={100}
            step={1}
            onChange={(v) => st.setMaskBrush({ flow: v })}
            onReset={() => st.setMaskBrush({ flow: 100 })}
          />
          <p className="panel-note">
            {t(
              chosen.shape.kind === "brush"
                ? "develop.masks.brush.hint"
                : "develop.masks.brush.eraseHint",
            )}
          </p>
          <h3 className="develop-subheading">{t("develop.masks.adjustments")}</h3>
          {LOCAL_KEYS.map((key) => {
            const v = chosen.adjust?.[key] ?? 0;
            const range = key === "exposure" ? LOCAL_EXPOSURE_RANGE : SLIDER_RANGE;
            return (
              <Slider
                key={key}
                label={t(`develop.${key}`)}
                display={format(key, v)}
                value={v}
                min={range.min}
                max={range.max}
                step={key === "exposure" ? 0.01 : 1}
                onChange={(value) => st.setMaskAdjust(selected, key, value)}
                onReset={() => st.setMaskAdjust(selected, key, 0)}
              />
            );
          })}
          <p className="panel-note">{t("develop.masks.selectedHint")}</p>
          <div className="develop-crop-actions">
            <span className="toolbar-spacer" />
            <button
              type="button"
              className="button button-small"
              disabled={full}
              onClick={() => void st.duplicateMask(selected)}
            >
              {t("develop.masks.duplicate")}
            </button>
            <button
              type="button"
              className="button button-small"
              onClick={() => void st.removeMask(selected)}
            >
              {t("develop.masks.remove")}
            </button>
          </div>
        </div>
      )}
    </PanelSection>
  );
}
