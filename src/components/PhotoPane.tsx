import { ImageOff } from "lucide-react";
import {
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent,
  type WheelEvent as ReactWheelEvent,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { useTranslation } from "react-i18next";
import { fullUrl } from "../api/previews";
import type { MediaSummary } from "../store/library";
import { displaySrc, preload } from "../ui/preload";

/* ---------- Orientation (pour le JPEG pleine taille, non orienté) ---------- */

/** Rotation et miroir correspondant à l'orientation EXIF (1 à 8). */
function orientationTransform(orientation: number): string {
  switch (orientation) {
    case 2:
      return "scaleX(-1)";
    case 3:
      return "rotate(180deg)";
    case 4:
      return "scaleY(-1)";
    case 5:
      return "rotate(90deg) scaleX(-1)";
    case 6:
      return "rotate(90deg)";
    case 7:
      return "rotate(270deg) scaleX(-1)";
    case 8:
      return "rotate(270deg)";
    default:
      return "none";
  }
}

const swapsAxes = (orientation: number) => orientation >= 5 && orientation <= 8;

interface Size {
  w: number;
  h: number;
}

interface Marquee {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

/** Rectangle réellement dessiné par une image en `object-fit: contain`. */
function containedRect(img: HTMLImageElement) {
  const box = img.getBoundingClientRect();
  const ratio = Math.min(box.width / img.naturalWidth, box.height / img.naturalHeight);
  const w = img.naturalWidth * ratio;
  const h = img.naturalHeight * ratio;
  return { left: box.left + (box.width - w) / 2, top: box.top + (box.height - h) / 2, w, h };
}

const clamp = (v: number, lo: number, hi: number) => Math.min(Math.max(v, lo), hi);

/**
 * Zoom de la visionneuse : point visé en fraction de l'image, et échelle par
 * rapport aux pixels réels (1, ou absente, = 100 %). `null` = ajusté.
 */
export type Zoom = { fx: number; fy: number; scale?: number } | null;

/** Échelle maximale (800 %) et pas de la molette. */
const MAX_SCALE = 8;
const WHEEL_STEP = 1.25;
/** Déplacement, en pixels, au-delà duquel un appui devient un tracé. */
const DRAG_THRESHOLD = 4;

interface PhotoPaneProps {
  media: MediaSummary;
  version: number;
  /** Zoom 100 % partagé (comparaison synchronisée) : point visé en fraction de l'image. */
  zoom: Zoom;
  onZoom: (zoom: Zoom) => void;
  /** Panneau actif : un clic zoome. Inactif : un clic l'active seulement. */
  active?: boolean;
  onActivate?: () => void;
  /** Appelé quand la photo affichée a changé (mesure de la visionneuse). */
  onShown?: () => void;
}

/**
 * Une photo ajustée à son cadre ; un clic zoome à 100 % sur le point cliqué,
 * un nouveau clic revient à l'ajusté. Le zoom à 100 % utilise le JPEG pleine
 * taille intégré au RAW ; l'aperçu agrandi le remplace en attendant.
 */
export function PhotoPane({
  media: current,
  version,
  zoom,
  onZoom: setZoom,
  active = true,
  onActivate,
  onShown,
}: PhotoPaneProps) {
  const { t } = useTranslation();
  const src = displaySrc(current, version);

  // L'image affichée ne change qu'une fois la suivante décodée : pas d'écran vide.
  const [shown, setShown] = useState<{ id: number; src: string } | null>(null);
  useEffect(() => {
    if (!src) return;
    let alive = true;
    const id = current.id;
    void preload(src).then(() => {
      if (!alive) return;
      setShown({ id, src });
      onShown?.();
    });
    return () => {
      alive = false;
    };
  }, [src, current.id, onShown]);

  const viewport = useRef<HTMLDivElement>(null);
  const [vp, setVp] = useState<Size>({ w: 0, h: 0 });
  useLayoutEffect(() => {
    const el = viewport.current;
    if (!el) return;
    const observer = new ResizeObserver(() => setVp({ w: el.clientWidth, h: el.clientHeight }));
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  // Taille naturelle du JPEG pleine taille, une fois chargé.
  const [full, setFull] = useState<{ id: number; size: Size } | null>(null);
  const [fullFailed, setFullFailed] = useState<number | null>(null);
  const fullSrc = zoom ? fullUrl(current.id) : null;
  const orientation = current.orientation;
  // 100 % = un pixel de l'image par pixel de l'écran, y compris en haute densité.
  const dpr = window.devicePixelRatio || 1;
  // Un JPEG intégré bien plus petit que le capteur (ARW, RW2) est agrandi aux
  // dimensions du capteur : même échelle pour tous les boîtiers en comparaison.
  const sensorLong = Math.max(current.width ?? 0, current.height ?? 0);
  const loaded = full && full.id === current.id ? full.size : null;
  const enlarge =
    loaded && sensorLong > 0 && Math.max(loaded.w, loaded.h) < sensorLong * 0.9
      ? sensorLong / Math.max(loaded.w, loaded.h)
      : 1;
  const fullSize = loaded ? { w: (loaded.w * enlarge) / dpr, h: (loaded.h * enlarge) / dpr } : null;
  const scale = zoom?.scale ?? 1;
  const actualPixels = loaded !== null && enlarge === 1;
  // Avant le chargement : dimensions du boîtier, orientées. Inconnues (photo
  // sans EXIF importée avant la lecture de l'en-tête) : la pleine taille,
  // chargée d'abord sans être montrée, les donne.
  const approx: Size | null =
    current.width && current.height
      ? swapsAxes(orientation)
        ? { w: current.height / dpr, h: current.width / dpr }
        : { w: current.width / dpr, h: current.height / dpr }
      : null;
  // Taille à 100 %, orientée.
  const base: Size | null = fullSize
    ? swapsAxes(orientation)
      ? { w: fullSize.h, h: fullSize.w }
      : fullSize
    : approx;
  const disp: Size | null = base ? { w: base.w * scale, h: base.h * scale } : null;
  // Échelle de l'ajusté : en deçà, on revient à l'ajusté.
  const fitScale = base && vp.w > 0 ? Math.min(vp.w / base.w, vp.h / base.h) : null;

  const fitImg = useRef<HTMLImageElement>(null);
  const drag = useRef<{ x: number; y: number; moved: boolean } | null>(null);
  // Zoom par rectangle, tracé sur la photo ajustée (coordonnées de la vue).
  const marqueeStart = useRef<{ x: number; y: number; id: number } | null>(null);
  const [marquee, setMarquee] = useState<Marquee | null>(null);

  const enterZoom = (e: ReactMouseEvent<HTMLImageElement>) => {
    if (!active) {
      onActivate?.();
      return;
    }
    const r = containedRect(e.currentTarget);
    setZoom({
      fx: clamp((e.clientX - r.left) / r.w, 0, 1),
      fy: clamp((e.clientY - r.top) / r.h, 0, 1),
    });
  };

  // Position du coin de l'image zoomée : le point visé au centre, sans laisser de vide.
  const place = (size: number, view: number, f: number) =>
    size <= view ? (view - size) / 2 : clamp(view / 2 - f * size, view - size, 0);

  /** Point de l'événement dans la vue. */
  const local = (e: { clientX: number; clientY: number }) => {
    const box = viewport.current?.getBoundingClientRect();
    return { x: e.clientX - (box?.left ?? 0), y: e.clientY - (box?.top ?? 0) };
  };

  const onPointerDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (!active) onActivate?.();
    if (e.button !== 0) return;
    if (!zoom) {
      // Ajusté : un tracé zoome sur le rectangle, un simple clic à 100 %.
      if (active && fitImg.current) marqueeStart.current = { ...local(e), id: e.pointerId };
      return;
    }
    drag.current = { x: e.clientX, y: e.clientY, moved: false };
    e.currentTarget.setPointerCapture(e.pointerId);
  };
  const onPointerMove = (e: ReactPointerEvent<HTMLDivElement>) => {
    const start = marqueeStart.current;
    if (start) {
      const p = local(e);
      if (marquee || Math.abs(p.x - start.x) + Math.abs(p.y - start.y) > DRAG_THRESHOLD) {
        // Capturé seulement une fois le tracé commencé : un simple clic
        // reste un clic sur la photo.
        if (!marquee) e.currentTarget.setPointerCapture(start.id);
        setMarquee({ x0: start.x, y0: start.y, x1: p.x, y1: p.y });
      }
      return;
    }
    const d = drag.current;
    if (!d || !zoom || !disp) return;
    const dx = e.clientX - d.x;
    const dy = e.clientY - d.y;
    if (Math.abs(dx) + Math.abs(dy) > 2) d.moved = true;
    d.x = e.clientX;
    d.y = e.clientY;
    setZoom({
      ...zoom,
      fx: clamp(zoom.fx - dx / disp.w, 0, 1),
      fy: clamp(zoom.fy - dy / disp.h, 0, 1),
    });
  };
  const onPointerUp = () => {
    marqueeStart.current = null;
    if (marquee) {
      setMarquee(null);
      zoomToMarquee(marquee);
      return;
    }
    const d = drag.current;
    drag.current = null;
    // Un clic sans déplacement revient à l'ajusté.
    if (d && !d.moved) setZoom(null);
  };
  const onPointerCancel = () => {
    marqueeStart.current = null;
    drag.current = null;
    setMarquee(null);
  };

  /** Rectangle tracé : sa zone remplit la vue. */
  const zoomToMarquee = (m: Marquee) => {
    const img = fitImg.current;
    const box = viewport.current?.getBoundingClientRect();
    if (!img || !box) return;
    const r = containedRect(img);
    const fx = (x: number) => clamp((x + box.left - r.left) / r.w, 0, 1);
    const fy = (y: number) => clamp((y + box.top - r.top) / r.h, 0, 1);
    const x0 = fx(Math.min(m.x0, m.x1));
    const x1 = fx(Math.max(m.x0, m.x1));
    const y0 = fy(Math.min(m.y0, m.y1));
    const y1 = fy(Math.max(m.y0, m.y1));
    if (x1 - x0 < 0.005 || y1 - y0 < 0.005) return;
    const center = { fx: (x0 + x1) / 2, fy: (y0 + y1) / 2 };
    if (!base || !fitScale) {
      setZoom(center);
      return;
    }
    const wanted = Math.min(vp.w / ((x1 - x0) * base.w), vp.h / ((y1 - y0) * base.h));
    setZoom({ ...center, scale: clamp(wanted, fitScale * WHEEL_STEP, MAX_SCALE) });
  };

  /** Molette : zoom progressif autour du point sous le pointeur. */
  const onWheel = (e: ReactWheelEvent<HTMLDivElement>) => {
    if (!active || e.deltaY === 0 || !base || !fitScale) return;
    const factor = e.deltaY < 0 ? WHEEL_STEP : 1 / WHEEL_STEP;
    if (!zoom) {
      if (factor < 1 || !fitImg.current) return;
      const r = containedRect(fitImg.current);
      setZoom({
        fx: clamp((e.clientX - r.left) / r.w, 0, 1),
        fy: clamp((e.clientY - r.top) / r.h, 0, 1),
        scale: Math.min(fitScale * WHEEL_STEP, MAX_SCALE),
      });
      return;
    }
    const next = Math.min(scale * factor, MAX_SCALE);
    if (next <= fitScale * 1.001) {
      setZoom(null);
      return;
    }
    // Le point sous le pointeur reste sous le pointeur.
    const p = local(e);
    const size = { w: base.w * scale, h: base.h * scale };
    const nextSize = { w: base.w * next, h: base.h * next };
    const ux = (p.x - place(size.w, vp.w, zoom.fx)) / size.w;
    const uy = (p.y - place(size.h, vp.h, zoom.fy)) / size.h;
    setZoom({
      fx: clamp((vp.w / 2 - (p.x - ux * nextSize.w)) / nextSize.w, 0, 1),
      fy: clamp((vp.h / 2 - (p.y - uy * nextSize.h)) / nextSize.h, 0, 1),
      scale: next,
    });
  };

  return (
    <div
      ref={viewport}
      className="loupe"
      data-zoomed={zoom ? true : undefined}
      data-active={active || undefined}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerCancel}
      onWheel={onWheel}
    >
      {!shown && !src && (
        <div className="loupe-empty">
          <ImageOff aria-hidden="true" strokeWidth={1.25} />
          <span>{t("loupe.preparing")}</span>
        </div>
      )}
      {shown && !zoom && (
        <img
          ref={fitImg}
          className="loupe-fit"
          src={shown.src}
          alt={current.fileName}
          draggable={false}
          onClick={enterZoom}
        />
      )}
      {marquee && (
        <div
          className="loupe-marquee"
          style={{
            left: Math.min(marquee.x0, marquee.x1),
            top: Math.min(marquee.y0, marquee.y1),
            width: Math.abs(marquee.x1 - marquee.x0),
            height: Math.abs(marquee.y1 - marquee.y0),
          }}
        />
      )}
      {shown && zoom && (
        <div
          className="loupe-zoomed"
          style={
            disp
              ? {
                  width: disp.w,
                  height: disp.h,
                  left: place(disp.w, vp.w, zoom.fx),
                  top: place(disp.h, vp.h, zoom.fy),
                }
              : { visibility: "hidden" }
          }
        >
          {/* Aperçu agrandi tant que la pleine taille charge (ou si elle manque). */}
          {disp && <img className="loupe-placeholder" src={shown.src} alt="" draggable={false} />}
          {fullSrc && fullFailed !== current.id && (
            <img
              className="loupe-full"
              src={fullSrc}
              alt={current.fileName}
              draggable={false}
              style={
                fullSize && disp
                  ? {
                      width: fullSize.w * scale,
                      height: fullSize.h * scale,
                      left: (disp.w - fullSize.w * scale) / 2,
                      top: (disp.h - fullSize.h * scale) / 2,
                      transform: orientationTransform(orientation),
                    }
                  : { visibility: "hidden" }
              }
              onLoad={(e) =>
                setFull({
                  id: current.id,
                  size: { w: e.currentTarget.naturalWidth, h: e.currentTarget.naturalHeight },
                })
              }
              onError={() => setFullFailed(current.id)}
            />
          )}
        </div>
      )}
      {zoom && (
        <span className="loupe-badge">
          {scale !== 1
            ? t("loupe.percent", { percent: Math.round(scale * 100) })
            : actualPixels
              ? t("loupe.actualPixels")
              : t("loupe.enlargedPreview")}
        </span>
      )}
    </div>
  );
}
