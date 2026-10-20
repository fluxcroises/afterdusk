import { type PointerEvent, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  SPOT_FEATHER_DEFAULT,
  SPOT_RADIUS,
  type Spot,
  type SpotMode,
  useDevelop,
} from "../store/develop";
import { outputSize, outputToSource, sourceToOutput } from "../ui/geometry";
import type { Box } from "./DevelopCrop";
import { Segmented } from "./DevelopAdvanced";
import { Slider } from "./DevelopPanels";
import { PanelSection } from "./PanelSection";

/** Partie visible de l'image rendue, en fractions (zoom 100 % : une région). */
export interface View {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

/** Distance (pixels de l'écran) sous laquelle un appui est un clic. */
const CLICK_SLOP = 4;

type Part = "target" | "source";

interface Press {
  x: number;
  y: number;
  moved: boolean;
  hit: { index: number; part: Part } | null;
}

/**
 * Corrections posées sur l'image : un clic corrige le point visé (source
 * choisie par le cœur), un glissé déplace une correction ou sa source, un
 * clic sur une correction la choisit. Au zoom, un glissé ailleurs déplace la
 * région. La position n'est enregistrée qu'au relâchement.
 */
export function SpotOverlay({ box, view }: { box: Box; view: View }) {
  const { t } = useTranslation();
  const spots = useDevelop((s) => s.settings?.spots) ?? [];
  const geometry = useDevelop((s) => s.settings?.geometry) ?? {};
  const source = useDevelop((s) => s.sourceSize);
  const selected = useDevelop((s) => s.selectedSpot);
  const brush = useDevelop((s) => s.spotBrush);
  const zoomed = useDevelop((s) => s.zoom !== null);
  const [press, setPress] = useState<Press | null>(null);
  const [draft, setDraft] = useState<[number, number] | null>(null);
  const [hover, setHover] = useState<[number, number] | null>(null);
  if (!source) return null;
  const { width: sw, height: sh } = source;

  const [outW] = outputSize(geometry, sw, sh);
  // Pixels de l'écran par pixel de la source.
  const scale = box.width / Math.max(1e-9, (view.x1 - view.x0) * outW);
  const toScreen = (sx: number, sy: number): [number, number] => {
    const [fx, fy] = sourceToOutput(sx, sy, geometry, sw, sh);
    return [
      ((fx - view.x0) / (view.x1 - view.x0)) * box.width,
      ((fy - view.y0) / (view.y1 - view.y0)) * box.height,
    ];
  };
  const toSource = (px: number, py: number): [number, number] =>
    outputToSource(
      view.x0 + (px / box.width) * (view.x1 - view.x0),
      view.y0 + (py / box.height) * (view.y1 - view.y0),
      geometry,
      sw,
      sh,
    );
  const radiusOf = (spot: Spot) => spot.radius * Math.max(sw, sh) * scale;

  const local = (e: PointerEvent<HTMLDivElement>): [number, number] => {
    const rect = e.currentTarget.getBoundingClientRect();
    return [e.clientX - rect.left, e.clientY - rect.top];
  };

  /** Correction (ou source de la correction choisie) sous le pointeur. */
  const hitAt = (px: number, py: number): Press["hit"] => {
    const near = (sx: number, sy: number, r: number) => {
      const [x, y] = toScreen(sx, sy);
      return Math.hypot(px - x, py - y) <= Math.max(r, 6);
    };
    const chosen = selected === null ? undefined : spots[selected];
    if (chosen && selected !== null && near(chosen.sourceX, chosen.sourceY, radiusOf(chosen))) {
      return { index: selected, part: "source" };
    }
    for (let i = spots.length - 1; i >= 0; i--) {
      const s = spots[i];
      if (s && near(s.x, s.y, radiusOf(s))) return { index: i, part: "target" };
    }
    return null;
  };

  const down = (e: PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    const [px, py] = local(e);
    setPress({ x: e.clientX, y: e.clientY, moved: false, hit: hitAt(px, py) });
  };

  const move = (e: PointerEvent<HTMLDivElement>) => {
    const [px, py] = local(e);
    setHover([px, py]);
    if (!press) return;
    const dx = e.clientX - press.x;
    const dy = e.clientY - press.y;
    if (!press.moved && Math.hypot(dx, dy) < CLICK_SLOP) return;
    if (press.hit) {
      setPress({ ...press, moved: true });
      setDraft(toSource(px, py));
    } else if (zoomed) {
      // Au zoom, glisser ailleurs que sur une correction déplace la région.
      const dpr = window.devicePixelRatio || 1;
      setPress({ ...press, x: e.clientX, y: e.clientY, moved: true });
      useDevelop.getState().panZoom(dx * dpr, dy * dpr);
    } else {
      setPress({ ...press, moved: true });
    }
  };

  const up = (e: PointerEvent<HTMLDivElement>) => {
    const p = press;
    setPress(null);
    setDraft(null);
    if (!p) return;
    const state = useDevelop.getState();
    if (p.hit) {
      if (p.moved && draft) void state.moveSpot(p.hit.index, p.hit.part, draft[0], draft[1]);
      else state.selectSpot(p.hit.index);
      return;
    }
    if (p.moved) return;
    const [px, py] = local(e);
    const [sx, sy] = toSource(px, py);
    if (sx >= 0 && sx <= 1 && sy >= 0 && sy <= 1) void state.addSpot(sx, sy);
  };

  const drawn = spots.map((s, i) => {
    if (press?.hit?.index === i && draft) {
      return press.hit.part === "target"
        ? { ...s, x: draft[0], y: draft[1] }
        : { ...s, sourceX: draft[0], sourceY: draft[1] };
    }
    return s;
  });
  const chosen = selected === null ? undefined : drawn[selected];
  const showBrush = hover && !press && !hitAt(hover[0], hover[1]);
  const brushRadius = brush.radius * Math.max(sw, sh) * scale;

  return (
    <div
      className="develop-spots"
      role="group"
      aria-label={t("develop.spots.title")}
      style={{ left: box.left, top: box.top, width: box.width, height: box.height }}
      data-dragging={(press?.moved ?? false) || undefined}
      onPointerDown={down}
      onPointerMove={move}
      onPointerUp={up}
      onPointerCancel={() => {
        setPress(null);
        setDraft(null);
      }}
      onPointerLeave={() => setHover(null)}
      onWheel={(e) => {
        // Molette : taille du pinceau (et de la correction choisie).
        const factor = e.deltaY < 0 ? 1.15 : 1 / 1.15;
        const radius = Math.min(SPOT_RADIUS.max, Math.max(SPOT_RADIUS.min, brush.radius * factor));
        useDevelop.getState().setSpotBrush({ radius: Math.round(radius * 1000) / 1000 });
      }}
    >
      <svg width={box.width} height={box.height} aria-hidden="true">
        {drawn.map((s, i) => {
          const [x, y] = toScreen(s.x, s.y);
          return (
            <circle
              key={i}
              className="develop-spot"
              data-selected={i === selected || undefined}
              cx={x}
              cy={y}
              r={radiusOf(s)}
            />
          );
        })}
        {chosen &&
          (() => {
            const [x, y] = toScreen(chosen.x, chosen.y);
            const [sx, sy] = toScreen(chosen.sourceX, chosen.sourceY);
            const r = radiusOf(chosen);
            const d = Math.hypot(sx - x, sy - y);
            // Trait d'un bord à l'autre, sans traverser les cercles.
            const k = d > 2 * r ? r / d : 0;
            return (
              <>
                {k > 0 && (
                  <line
                    className="develop-spot-link"
                    x1={x + (sx - x) * k}
                    y1={y + (sy - y) * k}
                    x2={sx - (sx - x) * k}
                    y2={sy - (sy - y) * k}
                  />
                )}
                <circle className="develop-spot develop-spot-source" cx={sx} cy={sy} r={r} />
              </>
            );
          })()}
        {showBrush && (
          <circle className="develop-spot-brush" cx={hover[0]} cy={hover[1]} r={brushRadius} />
        )}
      </svg>
    </div>
  );
}

/** Taille du pinceau affichée : le rayon en millièmes du côté long. */
const toSize = (radius: number) => Math.round(radius * 1000);

/** Panneau de l'outil : mode, taille, adoucissement, visualisation. */
export function SpotsSection() {
  const { t } = useTranslation();
  const brush = useDevelop((s) => s.spotBrush);
  const count = useDevelop((s) => s.settings?.spots?.length ?? 0);
  const selected = useDevelop((s) => s.selectedSpot);
  const showDust = useDevelop((s) => s.showDust);
  const st = useDevelop.getState();
  const modes: readonly SpotMode[] = ["heal", "clone"];

  return (
    <PanelSection
      id="develop.spots"
      title={t("develop.spots.title")}
      aside={<span className="panel-section-aside">{count > 0 ? count : ""}</span>}
    >
      <Segmented
        label={t("develop.spots.mode")}
        options={modes}
        value={brush.mode}
        onChange={(mode) => st.setSpotBrush({ mode })}
        text={(m) => t(`develop.spots.modes.${m}`)}
      />
      <Slider
        label={t("develop.spots.size")}
        display={String(toSize(brush.radius))}
        value={toSize(brush.radius)}
        min={toSize(SPOT_RADIUS.min)}
        max={toSize(SPOT_RADIUS.max)}
        step={1}
        onChange={(v) => st.setSpotBrush({ radius: v / 1000 })}
        onReset={() => st.setSpotBrush({ radius: 0.01 })}
      />
      <Slider
        label={t("develop.spots.feather")}
        display={String(brush.feather)}
        value={brush.feather}
        min={0}
        max={100}
        step={1}
        onChange={(v) => st.setSpotBrush({ feather: v })}
        onReset={() => st.setSpotBrush({ feather: SPOT_FEATHER_DEFAULT })}
      />
      <label className="check-row">
        <input
          type="checkbox"
          checked={showDust}
          onChange={(e) => st.setShowDust(e.currentTarget.checked)}
        />
        {t("develop.spots.visualize")}
      </label>
      <p className="panel-note">
        {t(selected === null ? "develop.spots.hint" : "develop.spots.selectedHint")}
      </p>
      <div className="develop-crop-actions">
        <span className="panel-note">{t("develop.spots.count", { count })}</span>
        <span className="toolbar-spacer" />
        {selected !== null && (
          <button
            type="button"
            className="button button-small"
            onClick={() => void st.removeSpot(selected)}
          >
            {t("develop.spots.remove")}
          </button>
        )}
        <button
          type="button"
          className="button button-small"
          disabled={count === 0}
          onClick={() => void st.clearSpots()}
        >
          {t("develop.spots.clear")}
        </button>
      </div>
    </PanelSection>
  );
}
