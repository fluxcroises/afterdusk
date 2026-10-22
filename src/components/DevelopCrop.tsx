import {
  Crop,
  FlipHorizontal2,
  FlipVertical2,
  Image,
  RotateCcw,
  RotateCcwSquare,
  RotateCwSquare,
  SquareDashed,
} from "lucide-react";
import { type PointerEvent, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  ASPECTS,
  type CropRect,
  FULL_CROP,
  aspectRatio,
  orientedAspect,
  useDevelop,
} from "../store/develop";
import { aspectLabel } from "../ui/aspects";
import { Slider } from "./DevelopPanels";
import { IconButton } from "./IconButton";
import { PanelSection } from "./PanelSection";

/** Boîte de l'image affichée, en pixels de la zone d'affichage. */
export interface Box {
  left: number;
  top: number;
  width: number;
  height: number;
}

type Grip = "move" | "n" | "s" | "e" | "w" | "ne" | "nw" | "se" | "sw";

/** Plus petit côté du cadre, en fraction de l'image. */
const MIN_SIDE = 0.02;

/** Nouveau cadre après un glissé de (dx, dy), en fractions de l'image. */
function dragged(
  start: CropRect,
  grip: Grip,
  dx: number,
  dy: number,
  ratio: number | null,
  boxAspect: number,
): CropRect | null {
  const c = { ...start };
  if (grip === "move") {
    const w = c.right - c.left;
    const h = c.bottom - c.top;
    const left = Math.min(1 - w, Math.max(0, c.left + dx));
    const top = Math.min(1 - h, Math.max(0, c.top + dy));
    return { left, top, right: left + w, bottom: top + h };
  }
  if (grip.includes("w")) c.left = start.left + dx;
  if (grip.includes("e")) c.right = start.right + dx;
  if (grip.includes("n")) c.top = start.top + dy;
  if (grip.includes("s")) c.bottom = start.bottom + dy;
  if (ratio !== null) {
    // Hauteur (en fraction) d'un cadre de ce ratio pour une largeur donnée.
    const heightFor = (w: number) => (w * boxAspect) / ratio;
    const w = c.right - c.left;
    const h = c.bottom - c.top;
    if (grip === "n" || grip === "s") {
      // Côté haut ou bas : la largeur suit, centrée.
      const nw = (h * ratio) / boxAspect;
      const cx = (start.left + start.right) / 2;
      c.left = cx - nw / 2;
      c.right = cx + nw / 2;
    } else {
      const nh = heightFor(w);
      if (grip === "e" || grip === "w") {
        const cy = (start.top + start.bottom) / 2;
        c.top = cy - nh / 2;
        c.bottom = cy + nh / 2;
      } else if (grip.includes("n")) {
        c.top = c.bottom - nh;
      } else {
        c.bottom = c.top + nh;
      }
    }
  }
  const ok =
    c.left >= -1e-6 &&
    c.top >= -1e-6 &&
    c.right <= 1 + 1e-6 &&
    c.bottom <= 1 + 1e-6 &&
    c.right - c.left >= MIN_SIDE &&
    c.bottom - c.top >= MIN_SIDE;
  return ok
    ? {
        left: Math.max(0, c.left),
        top: Math.max(0, c.top),
        right: Math.min(1, c.right),
        bottom: Math.min(1, c.bottom),
      }
    : null;
}

const GRIPS: Grip[] = ["nw", "n", "ne", "e", "se", "s", "sw", "w"];

/**
 * Cadre de recadrage posé sur l'image entière : coins et côtés pour le
 * redimensionner, glisser à l'intérieur pour le déplacer, extérieur assombri,
 * tiers en repère. Le cadre n'est enregistré qu'au relâchement.
 */
export function CropOverlay({ box }: { box: Box }) {
  const { t } = useTranslation();
  const stored = useDevelop((s) => s.settings?.geometry?.crop) ?? FULL_CROP;
  const aspect = useDevelop((s) => s.settings?.geometry?.aspect);
  const setCrop = useDevelop((s) => s.setCrop);
  const [draft, setDraft] = useState<CropRect>(stored);
  const [drag, setDrag] = useState<{ grip: Grip; x: number; y: number; start: CropRect } | null>(
    null,
  );

  const boxAspect = box.width / Math.max(1, box.height);
  // Pendant un glissé, le cadre en cours ; sinon celui des réglages.
  const current = drag ? draft : stored;
  // Ratio exact ; un ratio enregistré avant les ratios orientés suit le cadre.
  const ratio = aspectRatio(orientedAspect(aspect, current, boxAspect), boxAspect);

  const down = (grip: Grip) => (e: PointerEvent<HTMLDivElement>) => {
    e.stopPropagation();
    e.currentTarget.setPointerCapture(e.pointerId);
    setDraft(current);
    setDrag({ grip, x: e.clientX, y: e.clientY, start: current });
  };
  const move = (e: PointerEvent<HTMLDivElement>) => {
    if (!drag) return;
    const dx = (e.clientX - drag.x) / box.width;
    const dy = (e.clientY - drag.y) / box.height;
    const next = dragged(drag.start, drag.grip, dx, dy, ratio, boxAspect);
    if (next) setDraft(next);
  };
  const up = () => {
    if (!drag) return;
    setDrag(null);
    void setCrop(draft);
  };

  const pct = (v: number) => `${v * 100}%`;
  return (
    <div
      className="develop-crop"
      style={{ left: box.left, top: box.top, width: box.width, height: box.height }}
      onPointerMove={move}
      onPointerUp={up}
      onPointerCancel={up}
    >
      <div
        className="develop-crop-frame"
        role="group"
        aria-label={t("develop.crop.frame")}
        style={{
          left: pct(current.left),
          top: pct(current.top),
          width: pct(current.right - current.left),
          height: pct(current.bottom - current.top),
        }}
        onPointerDown={down("move")}
      >
        <div className="develop-crop-thirds" aria-hidden="true" />
        {GRIPS.map((g) => (
          <div
            key={g}
            className={`develop-crop-grip develop-crop-grip-${g}`}
            onPointerDown={down(g)}
          />
        ))}
      </div>
    </div>
  );
}

/** Forme d'un ratio, dessinée dans un carré de 18 px, comme dans Photos. */
function AspectShape({ aspect }: { aspect: string }) {
  if (aspect === "free") return <SquareDashed aria-hidden="true" strokeWidth={1.5} />;
  if (aspect === "original") return <Image aria-hidden="true" strokeWidth={1.5} />;
  const [a, b] = aspect.split(":").map(Number);
  const ratio = a && b ? a / b : 1;
  const w = ratio >= 1 ? 16 : 16 * ratio;
  const h = ratio >= 1 ? 16 / ratio : 16;
  return (
    <svg viewBox="0 0 18 18" aria-hidden="true">
      <rect x={9 - w / 2} y={9 - h / 2} width={w} height={h} rx={2} />
    </svg>
  );
}

/** Ratios en barre d'icônes : le cadre prend la forme choisie. */
function AspectStrip() {
  const { t } = useTranslation();
  const geometry = useDevelop((s) => s.settings?.geometry);
  const frame = useDevelop((s) => s.frame);
  const canvas = frame ? frame.width / frame.height : 1;
  const current = orientedAspect(geometry?.aspect, geometry?.crop ?? FULL_CROP, canvas);
  return (
    <div className="aspect-strip" role="group" aria-label={t("develop.crop.aspect")}>
      {ASPECTS.map((a) => (
        <button
          key={a}
          type="button"
          className="aspect-choice"
          aria-pressed={current === a}
          onClick={() => void useDevelop.getState().setAspect(a)}
        >
          <AspectShape aspect={a} />
          <span>{aspectLabel(t, a)}</span>
        </button>
      ))}
    </div>
  );
}

/** Panneau de l'outil : ouvrir, ratios, quarts de tour, miroirs, redresser. */
export function CropSection() {
  const { t, i18n } = useTranslation();
  const cropping = useDevelop((s) => s.cropping);
  const geometry = useDevelop((s) => s.settings?.geometry);
  const disabled = useDevelop((s) => s.settings === null);
  const busy = useDevelop((s) => s.autoBusy);
  const st = useDevelop.getState();
  const angle = geometry?.angle ?? 0;
  const aspect = geometry?.aspect ?? "free";
  const decimal = new Intl.NumberFormat(i18n.language, {
    signDisplay: "exceptZero",
    minimumFractionDigits: 1,
    maximumFractionDigits: 1,
  });
  return (
    <PanelSection id="develop.crop" title={t("develop.crop.title")}>
      {!cropping ? (
        <button
          type="button"
          className="button develop-crop-open"
          disabled={disabled}
          onClick={() => void st.startCrop()}
        >
          <Crop aria-hidden="true" strokeWidth={1.75} />
          {t("develop.crop.open")}
        </button>
      ) : (
        <>
          <AspectStrip />
          <div className="toolbar-group develop-crop-swap">
            <IconButton
              icon={RotateCcw}
              label={t("develop.crop.swap")}
              shortcut="X"
              disabled={aspect === "free" || aspect === "original" || aspect === "1:1"}
              onClick={() => void st.swapAspect()}
            />
            <span className="panel-note">{t("develop.crop.swap")}</span>
          </div>
          <div className="toolbar-group develop-crop-tools">
            <IconButton
              icon={RotateCcwSquare}
              label={t("develop.crop.rotateLeft")}
              onClick={() => st.rotateQuarter(-1)}
            />
            <IconButton
              icon={RotateCwSquare}
              label={t("develop.crop.rotateRight")}
              onClick={() => st.rotateQuarter(1)}
            />
            <IconButton
              icon={FlipHorizontal2}
              label={t("develop.crop.flipH")}
              pressed={geometry?.flipH ?? false}
              onClick={() => st.flip("h")}
            />
            <IconButton
              icon={FlipVertical2}
              label={t("develop.crop.flipV")}
              pressed={geometry?.flipV ?? false}
              onClick={() => st.flip("v")}
            />
          </div>
          <div className="develop-crop-straighten">
            <Slider
              label={t("develop.crop.straighten")}
              display={t("develop.crop.degrees", { value: decimal.format(angle) })}
              value={angle}
              min={-45}
              max={45}
              step={0.1}
              onChange={(v) => void st.setAngle(v)}
              onReset={() => void st.setAngle(0)}
            />
            <button
              type="button"
              className="button button-small"
              disabled={busy}
              title={t("develop.crop.autoHint")}
              onClick={() => void st.autoStraighten()}
            >
              {t("develop.auto")}
            </button>
          </div>
          <p className="panel-note">{t("develop.crop.hint")}</p>
          <div className="develop-crop-actions">
            <button type="button" className="button button-small" onClick={st.resetGeometry}>
              {t("develop.crop.reset")}
            </button>
            <span className="toolbar-spacer" />
            <button
              type="button"
              className="button button-small"
              onClick={() => void st.cancelCrop()}
            >
              {t("develop.crop.cancel")}
            </button>
            <button
              type="button"
              className="button button-small button-primary"
              onClick={st.finishCrop}
            >
              {t("develop.crop.done")}
            </button>
          </div>
        </>
      )}
    </PanelSection>
  );
}
