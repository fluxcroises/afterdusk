import { ImageOff } from "lucide-react";
import {
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { useTranslation } from "react-i18next";
import { fullUrl, previewUrl } from "../api/previews";
import { type MediaSummary, useLibrary } from "../store/library";
import { recordTransition } from "../ui/perf";
import { preload } from "../ui/preload";

const displaySrc = (m: MediaSummary, version: number) =>
  m.hasPreview ? previewUrl(m.id, 2048, version) : null;

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

/* ---------- Visionneuse ---------- */

interface Size {
  w: number;
  h: number;
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
 * Visionneuse : une photo à la fois, ajustée à l'espace de travail ; un clic
 * zoome à 100 % sur le point cliqué, un nouveau clic revient à l'ajusté.
 */
export function Loupe() {
  const { t } = useTranslation();
  const media = useLibrary((s) => s.media);
  const selectedId = useLibrary((s) => s.selectedId);
  const versions = useLibrary((s) => s.versions);
  const zoom = useLibrary((s) => s.zoom);
  const setZoom = useLibrary((s) => s.setZoom);
  const index = media.findIndex((m) => m.id === selectedId);
  const current = index >= 0 ? media[index] : undefined;
  const src = current ? displaySrc(current, versions[current.id] ?? 0) : null;

  // L'image affichée ne change qu'une fois la suivante décodée : pas d'écran vide.
  const [shown, setShown] = useState<{ id: number; src: string } | null>(null);
  useEffect(() => {
    if (!src || !current) return;
    let alive = true;
    const id = current.id;
    void preload(src).then(() => {
      if (!alive) return;
      setShown({ id, src });
      // Deux images d'animation : la nouvelle photo a été présentée au moins une
      // fois. La mesure majore donc le temps réel, ce qui convient à un plafond.
      requestAnimationFrame(() =>
        requestAnimationFrame(() => recordTransition(useLibrary.getState().navigatedAt)),
      );
    });
    return () => {
      alive = false;
    };
  }, [src, current]);

  // Les voisines sont décodées d'avance, la suivante d'abord.
  useEffect(() => {
    for (const d of [1, -1, 2, -2]) {
      const m = media[index + d];
      const next = m && displaySrc(m, versions[m.id] ?? 0);
      if (next) void preload(next);
    }
  }, [index, media, versions]);

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
  const fullSrc = current && zoom ? fullUrl(current.id) : null;
  const orientation = current?.orientation ?? 1;
  // 100 % = un pixel de l'image par pixel de l'écran, y compris en haute densité.
  const dpr = window.devicePixelRatio || 1;
  const fullSize =
    full && full.id === current?.id ? { w: full.size.w / dpr, h: full.size.h / dpr } : null;
  // Avant le chargement : dimensions du boîtier, orientées.
  const approx: Size | null =
    current?.width && current.height
      ? swapsAxes(orientation)
        ? { w: current.height / dpr, h: current.width / dpr }
        : { w: current.width / dpr, h: current.height / dpr }
      : null;
  const disp: Size | null = fullSize
    ? swapsAxes(orientation)
      ? { w: fullSize.h, h: fullSize.w }
      : fullSize
    : approx;

  const drag = useRef<{ x: number; y: number; moved: boolean } | null>(null);

  if (!current) return null;

  const enterZoom = (e: ReactMouseEvent<HTMLImageElement>) => {
    const r = containedRect(e.currentTarget);
    setZoom({
      fx: clamp((e.clientX - r.left) / r.w, 0, 1),
      fy: clamp((e.clientY - r.top) / r.h, 0, 1),
    });
  };

  const onPointerDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (!zoom) return;
    drag.current = { x: e.clientX, y: e.clientY, moved: false };
    e.currentTarget.setPointerCapture(e.pointerId);
  };
  const onPointerMove = (e: ReactPointerEvent<HTMLDivElement>) => {
    const d = drag.current;
    if (!d || !zoom || !disp) return;
    const dx = e.clientX - d.x;
    const dy = e.clientY - d.y;
    if (Math.abs(dx) + Math.abs(dy) > 2) d.moved = true;
    d.x = e.clientX;
    d.y = e.clientY;
    setZoom({
      fx: clamp(zoom.fx - dx / disp.w, 0, 1),
      fy: clamp(zoom.fy - dy / disp.h, 0, 1),
    });
  };
  const onPointerUp = () => {
    const d = drag.current;
    drag.current = null;
    // Un clic sans déplacement revient à l'ajusté.
    if (d && !d.moved) setZoom(null);
  };

  // Position du coin de l'image zoomée : le point visé au centre, sans laisser de vide.
  const place = (size: number, view: number, f: number) =>
    size <= view ? (view - size) / 2 : clamp(view / 2 - f * size, view - size, 0);

  return (
    <div
      ref={viewport}
      className="loupe"
      data-zoomed={zoom ? true : undefined}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
    >
      {!shown && !src && (
        <div className="loupe-empty">
          <ImageOff aria-hidden="true" strokeWidth={1.25} />
          <span>{t("loupe.preparing")}</span>
        </div>
      )}
      {shown && !zoom && (
        <img
          className="loupe-fit"
          src={shown.src}
          alt={current.fileName}
          draggable={false}
          onClick={enterZoom}
        />
      )}
      {shown && zoom && disp && (
        <div
          className="loupe-zoomed"
          style={{
            width: disp.w,
            height: disp.h,
            left: place(disp.w, vp.w, zoom.fx),
            top: place(disp.h, vp.h, zoom.fy),
          }}
        >
          {/* Aperçu agrandi tant que la pleine taille charge (ou si elle manque). */}
          <img className="loupe-placeholder" src={shown.src} alt="" draggable={false} />
          {fullSrc && fullFailed !== current.id && (
            <img
              className="loupe-full"
              src={fullSrc}
              alt={current.fileName}
              draggable={false}
              style={
                fullSize
                  ? {
                      width: fullSize.w,
                      height: fullSize.h,
                      left: (disp.w - fullSize.w) / 2,
                      top: (disp.h - fullSize.h) / 2,
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
          {fullSize ? t("loupe.actualPixels") : t("loupe.enlargedPreview")}
        </span>
      )}
    </div>
  );
}
