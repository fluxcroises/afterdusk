import { ImageOff, Images, SlidersHorizontal } from "lucide-react";
import { type PointerEvent, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { CatalogPanel } from "../components/CatalogPanels";
import {
  ColorSection,
  CompareControls,
  HistogramSection,
  HistorySection,
  LightSection,
} from "../components/DevelopPanels";
import {
  CurveSection,
  DetailSection,
  EffectsSection,
  HslSection,
  LensSection,
} from "../components/DevelopAdvanced";
import { type Box, CropOverlay, CropSection } from "../components/DevelopCrop";
import { FiltersPanel } from "../components/DevelopFilters";
import { SpotOverlay, SpotsSection, type View } from "../components/DevelopSpots";
import { DevelopToolRail } from "../components/DevelopTools";
import { SnapshotsSection, VersionsSection } from "../components/DevelopVersions";
import { EmptyState } from "../components/EmptyState";
import { Filmstrip } from "../components/Filmstrip";
import { ScreenLayout } from "../components/ScreenLayout";
import { type DevelopFrame, SPOT_RADIUS, useDevelop } from "../store/develop";
import { useLibrary } from "../store/library";

/** Champs qui ne prennent pas de texte : curseurs, cases à cocher, boutons. */
const NOT_TEXT = ["range", "checkbox", "radio", "button"];

/** Saisie de texte en cours (un curseur ou une case cochée n'en est pas une). */
function isTyping(target: EventTarget | null): boolean {
  if (target instanceof HTMLInputElement) return !NOT_TEXT.includes(target.type);
  return (
    target instanceof HTMLElement &&
    (target.isContentEditable || ["TEXTAREA", "SELECT"].includes(target.tagName))
  );
}

/** Distance (pixels de l'écran) sous laquelle un appui est un clic, pas un glissé. */
const CLICK_SLOP = 4;

/**
 * Image rendue par le cœur, dessinée telle quelle (aucun calcul de couleur
 * ici). Ajustée à l'écran, un clic zoome à 100 % sur le point visé ; au zoom,
 * un glissé déplace la région et un clic revient à l'image entière.
 */
function DevelopCanvas() {
  const { t } = useTranslation();
  const frame = useDevelop((s) => s.frame);
  const before = useDevelop((s) => s.beforeFrame);
  const compare = useDevelop((s) => s.compare);
  const picking = useDevelop((s) => s.picking);
  const zoom = useDevelop((s) => s.zoom);
  const zoomFrame = useDevelop((s) => s.zoomFrame);
  const ref = useRef<HTMLCanvasElement>(null);
  const press = useRef<{ x: number; y: number; moved: boolean } | null>(null);
  const [dragging, setDragging] = useState(false);
  const cropping = useDevelop((s) => s.cropping);
  const spotting = useDevelop((s) => s.tool === "spots");
  const fullSize = useDevelop((s) => s.fullSize);
  const zoomed = zoom !== null && zoomFrame !== null;
  const dpr = window.devicePixelRatio || 1;
  const [box, setBox] = useState<Box | null>(null);
  const overlay = cropping || spotting;

  // Boîte de l'image affichée, pour y poser le cadre de recadrage ou les
  // corrections.
  useEffect(() => {
    const canvas = ref.current;
    if (!canvas || !overlay) return;
    const measure = () =>
      setBox({
        left: canvas.offsetLeft,
        top: canvas.offsetTop,
        width: canvas.offsetWidth,
        height: canvas.offsetHeight,
      });
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(canvas);
    return () => observer.disconnect();
  }, [overlay, frame, zoomFrame]);

  // Partie de l'image rendue visible : l'image entière, ou la région zoomée.
  const view: View =
    zoomed && fullSize
      ? {
          x0: zoom.left / fullSize.width,
          y0: zoom.top / fullSize.height,
          x1: (zoom.left + zoomFrame.width) / fullSize.width,
          y1: (zoom.top + zoomFrame.height) / fullSize.height,
        }
      : { x0: 0, y0: 0, x1: 1, y1: 1 };

  useEffect(() => {
    const canvas = ref.current;
    const context = canvas?.getContext("2d");
    if (!canvas || !context || !frame) return;
    const image = (f: DevelopFrame) => new ImageData(f.pixels, f.width, f.height);
    if (zoomed) {
      // Un pixel de l'original par pixel de l'écran.
      canvas.width = zoomFrame.width;
      canvas.height = zoomFrame.height;
      canvas.style.width = `${zoomFrame.width / dpr}px`;
      canvas.style.height = `${zoomFrame.height / dpr}px`;
      context.putImageData(image(zoomFrame), 0, 0);
      return;
    }
    canvas.style.width = "";
    canvas.style.height = "";
    canvas.width = frame.width;
    canvas.height = frame.height;
    const sameSize = before && before.width === frame.width && before.height === frame.height;
    if (compare === "before" && sameSize) {
      context.putImageData(image(before), 0, 0);
      return;
    }
    context.putImageData(image(frame), 0, 0);
    if (compare === "split" && sameSize) {
      // Moitié gauche : l'original ; un trait marque la séparation.
      const half = Math.round(frame.width / 2);
      context.putImageData(image(before), 0, 0, 0, 0, half, frame.height);
      context.fillStyle = "rgb(255 255 255 / 0.85)";
      context.fillRect(half - 1, 0, 2, frame.height);
    }
  }, [frame, before, compare, zoomed, zoomFrame, dpr]);

  const onDown = (e: PointerEvent<HTMLCanvasElement>) => {
    if (cropping) return;
    press.current = { x: e.clientX, y: e.clientY, moved: false };
    if (zoomed && !picking) {
      e.currentTarget.setPointerCapture(e.pointerId);
      setDragging(true);
    }
  };

  const onMove = (e: PointerEvent<HTMLCanvasElement>) => {
    const p = press.current;
    if (!p || !dragging) return;
    const dx = e.clientX - p.x;
    const dy = e.clientY - p.y;
    if (!p.moved && Math.hypot(dx, dy) < CLICK_SLOP) return;
    press.current = { x: e.clientX, y: e.clientY, moved: true };
    useDevelop.getState().panZoom(dx * dpr, dy * dpr);
  };

  const onUp = (e: PointerEvent<HTMLCanvasElement>) => {
    const p = press.current;
    press.current = null;
    setDragging(false);
    if (!p || p.moved || !frame) return;
    const rect = e.currentTarget.getBoundingClientRect();
    const fx = (e.clientX - rect.left) / rect.width;
    const fy = (e.clientY - rect.top) / rect.height;
    const state = useDevelop.getState();
    if (picking) {
      // Pipette : le point visé, en pixels de l'aperçu.
      if (zoomed && state.fullSize) {
        const scale = frame.width / state.fullSize.width;
        void state.pickWhite(
          (zoom.left + fx * zoomFrame.width) * scale,
          (zoom.top + fy * zoomFrame.height) * scale,
        );
      } else {
        void state.pickWhite(fx * frame.width, fy * frame.height);
      }
    } else if (zoomed) {
      state.exitZoom();
    } else {
      void state.zoomAt(fx, fy);
    }
  };

  if (!frame) return null;
  return (
    <>
      <canvas
        ref={ref}
        className="develop-canvas"
        data-picking={picking || undefined}
        data-zoom={zoomed || undefined}
        data-dragging={dragging || undefined}
        aria-label={t("develop.image")}
        title={t(zoomed ? "develop.zoom.exitHint" : "develop.zoom.hint")}
        onPointerDown={onDown}
        onPointerMove={onMove}
        onPointerUp={onUp}
        onPointerCancel={() => {
          press.current = null;
          setDragging(false);
        }}
      />
      {cropping && box && <CropOverlay box={box} />}
      {spotting && box && <SpotOverlay box={box} view={view} />}
    </>
  );
}

/** Panneau droit : l'histogramme, puis l'outil choisi dans la barre. */
function SettingsPanel() {
  const tool = useDevelop((s) => s.tool);
  return (
    <>
      <HistogramSection />
      {tool === "adjust" && (
        <>
          <LightSection />
          <ColorSection />
          <CurveSection />
          <HslSection />
          <DetailSection />
          <EffectsSection />
          <LensSection />
        </>
      )}
      {tool === "filters" && <FiltersPanel />}
      {tool === "crop" && <CropSection />}
      {tool === "spots" && <SpotsSection />}
    </>
  );
}

/** Durée du rendu, moteur utilisé et décodage de l'original. */
function RenderStatus() {
  const { t } = useTranslation();
  const frame = useDevelop((s) => s.frame);
  const adapter = useDevelop((s) => s.adapter);
  const decodeMs = useDevelop((s) => s.decodeMs);
  const roundTripMs = useDevelop((s) => s.roundTripMs);
  const picking = useDevelop((s) => s.picking);
  const compare = useDevelop((s) => s.compare);
  const zoom = useDevelop((s) => s.zoom);
  const zoomFrame = useDevelop((s) => s.zoomFrame);
  const zoomLoading = useDevelop((s) => s.zoomLoading);
  if (picking) return <span className="toolbar-status">{t("develop.pickHint")}</span>;
  if (zoomLoading) return <span className="toolbar-status">{t("develop.zoom.loading")}</span>;
  if (zoom && zoomFrame) {
    return (
      <span className="toolbar-status">
        {t("develop.zoom.status", {
          render: zoomFrame.renderMs.toFixed(1),
          engine: zoomFrame.gpu && adapter ? adapter : t("develop.cpu"),
        })}
      </span>
    );
  }
  if (compare === "before") {
    return <span className="toolbar-status">{t("develop.compare.before")}</span>;
  }
  if (compare === "split") {
    return (
      <span className="toolbar-status">
        {t("develop.compare.before")} | {t("develop.compare.after")}
      </span>
    );
  }
  if (!frame) return null;
  return (
    <span className="toolbar-status">
      {t("develop.status", {
        engine: frame.gpu && adapter ? adapter : t("develop.cpu"),
        render: frame.renderMs.toFixed(1),
        total: Math.round(roundTripMs ?? 0),
        decode: decodeMs ?? 0,
        width: frame.width,
        height: frame.height,
      })}
    </span>
  );
}

export function DevelopStep() {
  const { t } = useTranslation();
  const selectedId = useLibrary((s) => s.selectedId);
  const count = useLibrary((s) => s.media.length);
  const loading = useDevelop((s) => s.loading);
  const error = useDevelop((s) => s.error);
  const view = useRef<HTMLDivElement>(null);

  // Taille de la zone d'affichage, en pixels de l'écran : celle de la région
  // rendue au zoom 100 %.
  useEffect(() => {
    const el = view.current;
    if (!el) return;
    const measure = () => {
      const cs = getComputedStyle(el);
      const w = el.clientWidth - parseFloat(cs.paddingLeft) - parseFloat(cs.paddingRight);
      const h = el.clientHeight - parseFloat(cs.paddingTop) - parseFloat(cs.paddingBottom);
      const dpr = window.devicePixelRatio || 1;
      useDevelop.getState().setViewport(Math.max(1, w * dpr), Math.max(1, h * dpr));
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  });

  useEffect(() => {
    if (selectedId !== null) void useDevelop.getState().open(selectedId);
  }, [selectedId]);
  // Quitter le Retoucher enregistre le dernier réglage et libère l'image.
  useEffect(() => () => void useDevelop.getState().close(), []);
  // « \ » : l'original ou la retouche, comme dans Lightroom (AltGr+8 en AZERTY).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (isTyping(e.target)) return;
      const state = useDevelop.getState();
      if (e.key === "\\") {
        state.setCompare(state.compare === "before" ? "off" : "before");
        e.preventDefault();
        return;
      }
      // La lettre tapée (Z en AZERTY est la touche KeyW) ; la position de la
      // touche pour les claviers non latins.
      const letter = (k: string) =>
        /^[a-z]$/i.test(e.key) ? e.key.toLowerCase() === k : e.code === `Key${k.toUpperCase()}`;
      const plain = !e.ctrlKey && !e.metaKey && !e.altKey;
      // R : outil de recadrage ; Entrée l'applique, Échap l'annule ; X change
      // l'orientation du ratio.
      if (plain && letter("r")) {
        if (state.cropping) state.finishCrop();
        else void state.setTool("crop");
        e.preventDefault();
        return;
      }
      // Q : correcteur de défauts. Suppr retire la correction choisie, [ et ]
      // changent la taille du pinceau, Échap quitte la correction choisie
      // puis l'outil.
      if (plain && letter("q")) {
        void state.setTool(state.tool === "spots" ? "adjust" : "spots");
        e.preventDefault();
        return;
      }
      if (state.tool === "spots") {
        if ((e.key === "Delete" || e.key === "Backspace") && state.selectedSpot !== null) {
          void state.removeSpot(state.selectedSpot);
          e.preventDefault();
          return;
        }
        if (e.key === "Escape") {
          if (state.selectedSpot !== null) state.selectSpot(null);
          else void state.setTool("adjust");
          e.preventDefault();
          return;
        }
        if (e.key === "[" || e.key === "]") {
          const factor = e.key === "]" ? 1.2 : 1 / 1.2;
          const radius = Math.min(
            SPOT_RADIUS.max,
            Math.max(SPOT_RADIUS.min, state.spotBrush.radius * factor),
          );
          state.setSpotBrush({ radius: Math.round(radius * 1000) / 1000 });
          e.preventDefault();
          return;
        }
      }
      if (state.cropping && e.key === "Enter") {
        state.finishCrop();
        e.preventDefault();
        return;
      }
      if (state.cropping && e.key === "Escape") {
        void state.cancelCrop();
        e.preventDefault();
        return;
      }
      const aspect = state.settings?.geometry?.aspect;
      if (state.cropping && plain && letter("x") && aspect && aspect !== "free") {
        const c = state.settings?.geometry?.crop ?? { left: 0, top: 0, right: 1, bottom: 1 };
        void state.setAspect(aspect, !(c.bottom - c.top > c.right - c.left));
        e.preventDefault();
        return;
      }
      // Z : zoom 100 % au centre, ou retour à l'image entière.
      if (plain && letter("z")) {
        if (state.zoom) state.exitZoom();
        else void state.zoomAt(0.5, 0.5);
        e.preventDefault();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return (
    <ScreenLayout
      left={
        <>
          <VersionsSection />
          <SnapshotsSection />
          <HistorySection />
          <CatalogPanel scope="develop" />
        </>
      }
      right={<SettingsPanel />}
      rail={<DevelopToolRail />}
      toolbar={
        <>
          <CompareControls />
          <span className="toolbar-spacer" />
          <RenderStatus />
        </>
      }
      filmstrip={<Filmstrip />}
    >
      {count === 0 ? (
        <EmptyState icon={Images} title={t("develop.emptyTitle")}>
          {t("develop.emptyText")}
        </EmptyState>
      ) : selectedId === null ? (
        <EmptyState icon={SlidersHorizontal} title={t("develop.chooseTitle")}>
          {t("develop.chooseText")}
        </EmptyState>
      ) : error ? (
        <EmptyState icon={ImageOff} title={t("develop.errorTitle")}>
          {t(`errors.${error.code}`, { defaultValue: t("errors.internal") })}
        </EmptyState>
      ) : (
        <div className="develop-view" ref={view} aria-busy={loading}>
          {loading && <p className="develop-loading">{t("develop.loading")}</p>}
          <DevelopCanvas />
        </div>
      )}
    </ScreenLayout>
  );
}
