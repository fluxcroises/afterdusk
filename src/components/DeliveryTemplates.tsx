import { Crop, LayoutPanelLeft, LayoutTemplate, Pencil, TriangleAlert } from "lucide-react";
import {
  type CSSProperties,
  type KeyboardEvent,
  type PointerEvent,
  useEffect,
  useRef,
  useState,
} from "react";
import { useTranslation } from "react-i18next";
import { previewUrl } from "../api/previews";
import { useDesigns } from "../store/designs";
import type { CropRect } from "../store/develop";
import { useLibrary } from "../store/library";
import { type Grip, dragged } from "../ui/crop";
import { type DeliveryTemplate, type Framing, GROUPS, useTemplates } from "../store/templates";
import { EmptyState } from "./EmptyState";
import { IconButton } from "./IconButton";

const CORNERS: Grip[] = ["nw", "ne", "se", "sw"];

const toCrop = ([left, top, right, bottom]: Framing["rect"]): CropRect => ({
  left,
  top,
  right,
  bottom,
});

/** Photo vue à travers un cadrage : l'image agrandie et décalée par CSS. */
function Framed({ src, rect }: { src: string | null; rect: Framing["rect"] }) {
  const [l, t, r, b] = rect;
  const w = Math.max(r - l, 0.001);
  const h = Math.max(b - t, 0.001);
  return src ? (
    <img
      src={src}
      alt=""
      draggable={false}
      style={{
        width: `${100 / w}%`,
        height: `${100 / h}%`,
        left: `${(-l / w) * 100}%`,
        top: `${(-t / h) * 100}%`,
      }}
    />
  ) : null;
}

function TemplateCard({
  template,
  framing,
  src,
}: {
  template: DeliveryTemplate;
  framing: Framing | undefined;
  src: string | null;
}) {
  const { t } = useTranslation();
  const chosen = useTemplates((s) => s.chosen.includes(template.id));
  const st = useTemplates.getState();
  const name = t(`templates.names.${template.id}`);
  const width = framing?.width ?? template.width;
  const height = framing?.height ?? template.height;
  const open = () => {
    st.choose(template.id);
    st.setEditing(template.id);
  };
  // Composition : la photo active d'abord, puis le reste de la sélection.
  const compose = () => {
    const lib = useLibrary.getState();
    const items =
      lib.selectedId === null
        ? lib.selectedIds
        : [lib.selectedId, ...lib.selectedIds.filter((i) => i !== lib.selectedId)];
    void useDesigns.getState().create(template.id, width, height, items, name);
  };
  return (
    <li className="template-card" data-chosen={chosen || undefined}>
      <button
        type="button"
        className="template-pick"
        aria-pressed={chosen}
        onClick={(e) => e.detail < 2 && st.toggle(template.id)}
        onDoubleClick={open}
      >
        <span className="template-frame" style={{ "--ratio": width / height } as CSSProperties}>
          {framing && <Framed src={src} rect={framing.rect} />}
        </span>
        <span className="template-name">{name}</span>
        <span className="template-size">{t("templates.size", { width, height })}</span>
        {framing && !framing.sharpEnough && (
          <span className="template-warning">
            <TriangleAlert aria-hidden="true" strokeWidth={1.75} />
            {t("templates.lowResolution")}
          </span>
        )}
        {framing?.custom && <span className="template-custom">{t("templates.custom")}</span>}
      </button>
      <span className="template-edit">
        <IconButton icon={Crop} label={t("templates.frame", { name })} onClick={open} />
        <IconButton
          icon={LayoutPanelLeft}
          label={t("design.compose", { name })}
          onClick={compose}
        />
      </span>
    </li>
  );
}

/** Compositions enregistrées : rouvrir pour modifier ou exporter. */
function SavedDesigns() {
  const { t } = useTranslation();
  const saved = useDesigns((s) => s.saved);
  if (saved.length === 0) return null;
  return (
    <section className="templates-group" aria-labelledby="templates-designs">
      <h2 id="templates-designs" className="templates-group-title">
        {t("design.saved")}
      </h2>
      <ul className="design-saved">
        {saved.map((d) => (
          <li key={d.id}>
            <span className="design-saved-name">{d.name}</span>
            <span className="template-size">
              {t(`templates.names.${d.document.template}`)} ·{" "}
              {t("design.photos", { count: d.document.photos.length })}
            </span>
            <IconButton
              icon={Pencil}
              label={t("design.open", { name: d.name })}
              onClick={() => useDesigns.getState().open(d)}
            />
          </li>
        ))}
      </ul>
    </section>
  );
}

/**
 * Modèles de livraison (V0.5) : la photo active vue dans chaque format,
 * rangés par usage. Un clic coche le modèle pour l'export, un double-clic
 * ouvre son cadrage.
 */
export function TemplatesView() {
  const { t } = useTranslation();
  const item = useLibrary((s) => s.selectedId);
  const version = useLibrary((s) => (s.selectedId === null ? 0 : (s.versions[s.selectedId] ?? 0)));
  const many = useLibrary((s) => s.selectedIds.length > 1);
  const templates = useTemplates((s) => s.templates);
  const framings = useTemplates((s) => s.framings);
  const editing = useTemplates((s) => s.editing);

  useEffect(() => {
    void useTemplates.getState().load(item);
  }, [item, version]);
  useEffect(() => {
    void useDesigns.getState().load();
  }, []);

  if (item === null) {
    return <EmptyState icon={LayoutTemplate} title={t("templates.needPhoto")} />;
  }
  const src = previewUrl(item, 1024, version);
  const byId = new Map(framings.map((f) => [f.template, f]));
  const editingFraming = editing ? byId.get(editing) : undefined;
  return (
    <div className="templates-view">
      <p className="panel-note templates-hint">
        {t(many ? "templates.hintMany" : "templates.hint")}
      </p>
      <SavedDesigns />
      {GROUPS.map((group) => {
        const list = templates.filter((tp) => tp.group === group);
        if (list.length === 0) return null;
        return (
          <section key={group} className="templates-group" aria-labelledby={`templates-${group}`}>
            <h2 id={`templates-${group}`} className="templates-group-title">
              {t(`templates.groups.${group}`)}
            </h2>
            <ul className="templates-grid">
              {list.map((tp) => (
                <TemplateCard key={tp.id} template={tp} framing={byId.get(tp.id)} src={src} />
              ))}
            </ul>
          </section>
        );
      })}
      {editingFraming && (
        <FramingDialog
          key={`${item}-${editing}`}
          framing={editingFraming}
          src={previewUrl(item, 2048, version)}
        />
      )}
    </div>
  );
}

/** Pas des flèches du clavier, en fraction de l'image (Maj : dix fois plus). */
const KEY_STEP = 0.01;

/**
 * Cadrage d'une photo pour un modèle : le cadre garde le rapport du modèle,
 * se déplace en glissant ou aux flèches, se redimensionne par les coins.
 */
function FramingDialog({ framing, src }: { framing: Framing; src: string | null }) {
  const { t } = useTranslation();
  const st = useTemplates.getState();
  const stage = useRef<HTMLDivElement>(null);
  const [aspect, setAspect] = useState<number | null>(null);
  const [rect, setRect] = useState<CropRect>(toCrop(framing.rect));
  const [drag, setDrag] = useState<{ grip: Grip; x: number; y: number; start: CropRect } | null>(
    null,
  );
  const ratio = framing.width / framing.height;
  const name = t(`templates.names.${framing.template}`);
  const close = () => st.setEditing(null);
  const save = () => {
    void st.save(framing.template, [rect.left, rect.top, rect.right, rect.bottom]);
    close();
  };

  useEffect(() => {
    const key = (e: globalThis.KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.stopPropagation();
      st.setEditing(null);
    };
    window.addEventListener("keydown", key, true);
    return () => window.removeEventListener("keydown", key, true);
  }, [st]);

  const box = () => stage.current?.getBoundingClientRect();
  const down = (grip: Grip) => (e: PointerEvent<HTMLDivElement>) => {
    e.stopPropagation();
    e.currentTarget.setPointerCapture(e.pointerId);
    setDrag({ grip, x: e.clientX, y: e.clientY, start: rect });
  };
  const move = (e: PointerEvent<HTMLDivElement>) => {
    const b = box();
    if (!drag || !b) return;
    const next = dragged(
      drag.start,
      drag.grip,
      (e.clientX - drag.x) / b.width,
      (e.clientY - drag.y) / b.height,
      ratio,
      b.width / Math.max(1, b.height),
    );
    if (next) setRect(next);
  };
  const keys = (e: KeyboardEvent<HTMLDivElement>) => {
    const step = e.shiftKey ? KEY_STEP * 10 : KEY_STEP;
    const d = {
      ArrowLeft: [-step, 0],
      ArrowRight: [step, 0],
      ArrowUp: [0, -step],
      ArrowDown: [0, step],
    }[e.key];
    if (!d) return;
    e.preventDefault();
    const next = dragged(rect, "move", d[0], d[1], null, 1);
    if (next) setRect(next);
  };

  const pct = (v: number) => `${v * 100}%`;
  return (
    <div className="modal-backdrop" onMouseDown={(e) => e.target === e.currentTarget && close()}>
      <form
        className="copy-dialog framing-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="framing-title"
        onSubmit={(e) => {
          e.preventDefault();
          save();
        }}
      >
        <h2 id="framing-title" className="copy-title">
          {t("templates.frameTitle", { name })}
        </h2>
        <div className="framing-area">
          <div
            ref={stage}
            className="framing-stage"
            style={{ "--aspect": aspect ?? 1.5 } as CSSProperties}
            onPointerMove={move}
            onPointerUp={() => setDrag(null)}
            onPointerCancel={() => setDrag(null)}
          >
            {src && (
              <img
                src={src}
                alt=""
                draggable={false}
                onLoad={(e) =>
                  setAspect(
                    e.currentTarget.naturalWidth / Math.max(1, e.currentTarget.naturalHeight),
                  )
                }
              />
            )}
            {aspect && (
              <div className="develop-crop framing-crop">
                <div
                  className="develop-crop-frame"
                  role="group"
                  tabIndex={0}
                  aria-label={t("templates.frameArea")}
                  style={{
                    left: pct(rect.left),
                    top: pct(rect.top),
                    width: pct(rect.right - rect.left),
                    height: pct(rect.bottom - rect.top),
                  }}
                  onPointerDown={down("move")}
                  onKeyDown={keys}
                >
                  <div className="develop-crop-thirds" aria-hidden="true" />
                  {CORNERS.map((g) => (
                    <div
                      key={g}
                      className={`develop-crop-grip develop-crop-grip-${g}`}
                      onPointerDown={down(g)}
                    />
                  ))}
                </div>
              </div>
            )}
          </div>
        </div>
        <p className="panel-note">{t("templates.frameHint")}</p>
        <div className="develop-crop-actions">
          <button
            type="button"
            className="button button-small"
            disabled={!framing.custom}
            onClick={() => {
              void st.reset(framing.template);
              close();
            }}
          >
            {t("templates.resetFrame")}
          </button>
          <span className="toolbar-spacer" />
          <button type="button" className="button button-small" onClick={close}>
            {t("templates.cancel")}
          </button>
          <button type="submit" className="button button-small button-primary">
            {t("templates.save")}
          </button>
        </div>
      </form>
    </div>
  );
}
