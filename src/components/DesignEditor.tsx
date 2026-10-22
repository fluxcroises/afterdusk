import { useAllows } from "../store/profile";
import type Konva from "konva";
import { Plus, RotateCw, Type, X } from "lucide-react";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Group, Image as KImage, Layer, Rect, Stage, Text } from "react-konva";
import { previewUrl } from "../api/previews";
import { useDesigns } from "../store/designs";
import { useLibrary } from "../store/library";
import { useStudioLogo } from "../store/studioLogo";
import { confirmWarning } from "../ui/confirm";
import {
  type Cell,
  DEFAULT_FONT,
  type Design,
  type DesignPhoto,
  type DesignText,
  LINE_HEIGHT,
  cells,
  columns,
  cover,
  inkFor,
  maxPhotos,
} from "../ui/design";
import { Slider } from "./DevelopPanels";
import { ErrorBanner } from "./ErrorBanner";
import { IconButton } from "./IconButton";

/** Place de la page dans l'éditeur, en pixels d'écran. */
const STAGE_MAX: [number, number] = [760, 560];

const NEW_TEXT: DesignText = {
  text: "",
  x: 0.5,
  y: 0.4,
  size: 8,
  font: DEFAULT_FONT,
  weight: 600,
  color: "#ffffff",
  align: "center",
};

const WEIGHTS = [300, 400, 600, 700, 800];

/** Image chargée pour Konva ; `null` tant qu'elle ne l'est pas. */
function useImage(src: string | null): HTMLImageElement | null {
  const [loaded, setLoaded] = useState<{ src: string; img: HTMLImageElement } | null>(null);
  useEffect(() => {
    if (!src) return;
    const img = new window.Image();
    img.onload = () => setLoaded({ src, img });
    img.src = src;
    return () => {
      img.onload = null;
    };
  }, [src]);
  return loaded && loaded.src === src ? loaded.img : null;
}

/** Une photo dans sa case : glisser la déplace, la case la garde remplie. */
function PhotoCell({
  photo,
  cell,
  page,
  scale,
  onFocus,
}: {
  photo: DesignPhoto;
  cell: Cell;
  page: [number, number];
  scale: number;
  onFocus: (focus: [number, number]) => void;
}) {
  const img = useImage(previewUrl(photo.item, 2048));
  const [x, y] = [cell[0] * page[0] * scale, cell[1] * page[1] * scale];
  const [w, h] = [(cell[2] - cell[0]) * page[0] * scale, (cell[3] - cell[1]) * page[1] * scale];
  if (!img) return <Rect x={x} y={y} width={w} height={h} fill="#2c3036" />;
  const aspect = img.naturalWidth / Math.max(1, img.naturalHeight);
  const [l, t, r, b] = cover(w / Math.max(1, h), aspect, photo.focus);
  const iw = w / (r - l);
  const ih = h / (b - t);
  return (
    <Group clipX={x} clipY={y} clipWidth={w} clipHeight={h}>
      <KImage
        image={img}
        x={x - l * iw}
        y={y - t * ih}
        width={iw}
        height={ih}
        draggable
        dragBoundFunc={(pos) => ({
          x: Math.min(x, Math.max(x + w - iw, pos.x)),
          y: Math.min(y, Math.max(y + h - ih, pos.y)),
        })}
        onDragEnd={(e) => {
          const nl = (x - e.target.x()) / iw;
          const nt = (y - e.target.y()) / ih;
          onFocus([nl + (r - l) / 2, nt + (b - t) / 2]);
        }}
      />
    </Group>
  );
}

function TextNode({
  text,
  index,
  page,
  scale,
  selected,
  shift = 0,
}: {
  text: DesignText;
  index: number;
  page: [number, number];
  scale: number;
  selected: boolean;
  /** Copie sur une autre bande : décalage en pixels, ni sélection ni glisser. */
  shift?: number;
}) {
  const ref = useRef<Konva.Text>(null);
  const k = text.align === "center" ? 0.5 : text.align === "right" ? 1 : 0;
  // Konva mesure le texte au dessin : on le redessine une fois la police
  // chargée par le navigateur intégré.
  const [loaded, setLoaded] = useState(0);
  useEffect(() => {
    if (!("fonts" in document)) return;
    let live = true;
    void document.fonts
      .load(`${text.weight} 32px "${text.font}"`)
      .then(() => live && setLoaded((n) => n + 1));
    return () => {
      live = false;
    };
  }, [text.font, text.weight]);
  // L'accroche est le bord ou le milieu de la ligne la plus longue.
  useLayoutEffect(() => {
    const node = ref.current;
    if (node) node.offsetX(node.width() * k);
  });
  const st = useDesigns.getState();
  return (
    <Text
      key={loaded}
      ref={ref}
      text={text.text || " "}
      x={text.x * page[0] * scale + shift}
      y={text.y * page[1] * scale}
      listening={shift === 0}
      fontFamily={`"${text.font}", Inter`}
      fontSize={(text.size / 100) * page[1] * scale}
      fontStyle={String(text.weight)}
      lineHeight={LINE_HEIGHT}
      align={text.align}
      fill={text.color}
      stroke={selected && shift === 0 ? "#8ab4ff" : undefined}
      strokeWidth={selected && shift === 0 ? 0.6 : 0}
      draggable={shift === 0}
      onMouseDown={() => st.select(index)}
      onDragEnd={(e) => {
        const texts = [...(st.draft?.document.texts ?? [])];
        texts[index] = {
          ...text,
          x: e.target.x() / (page[0] * scale),
          y: e.target.y() / (page[1] * scale),
        };
        st.update({ texts });
      }}
    />
  );
}

function LogoNode({ design, scale, shift = 0 }: { design: Design; scale: number; shift?: number }) {
  const url = useStudioLogo((s) => s.url);
  const img = useImage(url);
  const logo = design.logo;
  if (!logo || !img) return null;
  const w = (logo.width / 100) * design.width * scale;
  const h = (w * img.naturalHeight) / Math.max(1, img.naturalWidth);
  const st = useDesigns.getState();
  return (
    <KImage
      image={img}
      x={logo.x * design.width * scale + shift}
      y={logo.y * design.height * scale}
      width={w}
      height={h}
      opacity={logo.opacity / 100}
      listening={shift === 0}
      draggable={shift === 0}
      onMouseDown={() => st.select("logo")}
      onDragEnd={(e) =>
        st.update({
          logo: {
            ...logo,
            x: e.target.x() / (design.width * scale),
            y: e.target.y() / (design.height * scale),
          },
        })
      }
    />
  );
}

/** La page telle qu'elle sera exportée, à l'échelle de l'écran. */
function Page({ design }: { design: Design }) {
  const selected = useDesigns((s) => s.selected);
  const st = useDesigns.getState();
  const scale = Math.min(STAGE_MAX[0] / design.width, STAGE_MAX[1] / design.height);
  const page: [number, number] = [design.width, design.height];
  const [w, h] = [design.width * scale, design.height * scale];
  const n = design.photos.length;
  const boxes = cells(design.layout, n, design.width / design.height, design.margin);
  const short = Math.min(design.width, design.height) * scale;
  const repeat = columns(design.layout, design.width / design.height);
  return (
    <Stage width={w} height={h} className="design-stage">
      <Layer>
        <Rect width={w} height={h} fill={design.background} onMouseDown={() => st.select(null)} />
        {boxes.map((cell, k) => {
          // Une bande doublée répète ses photos dans la seconde colonne.
          const i = k % n;
          const p = design.photos[i];
          return (
            <PhotoCell
              key={`${p.item}-${k}`}
              photo={p}
              cell={cell}
              page={page}
              scale={scale}
              onFocus={(focus) => {
                const photos = [...design.photos];
                photos[i] = { ...p, focus };
                st.update({ photos });
              }}
            />
          );
        })}
        {design.frame &&
          boxes.map((c, i) => {
            const t = ((design.frame?.width ?? 0) / 100) * short;
            return (
              <Rect
                key={i}
                x={c[0] * w + t / 2}
                y={c[1] * h + t / 2}
                width={(c[2] - c[0]) * w - t}
                height={(c[3] - c[1]) * h - t}
                stroke={design.frame?.color}
                strokeWidth={t}
                listening={false}
              />
            );
          })}
        {/* Bande doublée : textes et logo répétés sur chaque bande. */}
        {Array.from({ length: repeat }, (_, c) => (
          <Group key={c}>
            <LogoNode design={design} scale={scale} shift={(c * w) / repeat} />
            {design.texts.map((tx, i) => (
              <TextNode
                key={i}
                text={tx}
                index={i}
                page={page}
                scale={scale}
                selected={selected === i}
                shift={(c * w) / repeat}
              />
            ))}
          </Group>
        ))}
      </Layer>
    </Stage>
  );
}

function Color({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
}) {
  return (
    <label className="deliver-field design-color">
      <span>{label}</span>
      <input type="color" value={value} onChange={(e) => onChange(e.currentTarget.value)} />
    </label>
  );
}

function TextControls({ index, text }: { index: number; text: DesignText }) {
  const { t } = useTranslation();
  const st = useDesigns.getState();
  const texts = useDesigns((s) => s.draft?.document.texts ?? []);
  const fonts = useDesigns((s) => s.fonts);
  const systemFonts = useAllows("design.systemFonts");
  const set = (patch: Partial<DesignText>) => {
    const next = [...texts];
    next[index] = { ...text, ...patch };
    st.update({ texts: next });
  };
  const known = (list: string[]) => list.some((f) => f.toLowerCase() === text.font.toLowerCase());
  // Composition faite sur un autre ordinateur : la police peut manquer ici.
  const missing = fonts !== null && !known(fonts.bundled) && !known(fonts.system);
  return (
    <div className="design-group">
      <label className="deliver-field design-textarea">
        <span>{t("design.text")}</span>
        <textarea
          rows={2}
          value={text.text}
          placeholder={t("design.textPlaceholder")}
          onChange={(e) => set({ text: e.currentTarget.value })}
        />
      </label>
      <label className="deliver-field">
        <span>{t("design.font")}</span>
        <select value={text.font} onChange={(e) => set({ font: e.currentTarget.value })}>
          {missing && <option value={text.font}>{text.font}</option>}
          <optgroup label={t("design.bundledFonts")}>
            {(fonts?.bundled ?? [DEFAULT_FONT]).map((f) => (
              <option key={f} value={f}>
                {f}
              </option>
            ))}
          </optgroup>
          {systemFonts && fonts && fonts.system.length > 0 && (
            <optgroup label={t("design.systemFonts")}>
              {fonts.system.map((f) => (
                <option key={f} value={f}>
                  {f}
                </option>
              ))}
            </optgroup>
          )}
        </select>
      </label>
      {missing && <p className="panel-note">{t("design.missingFont", { font: text.font })}</p>}
      <Slider
        label={t("design.textSize")}
        value={text.size}
        min={1}
        max={30}
        step={0.5}
        display={`${text.size} %`}
        onChange={(size) => set({ size })}
        onReset={() => set({ size: 8 })}
      />
      <label className="deliver-field">
        <span>{t("design.weight")}</span>
        <select
          value={text.weight}
          onChange={(e) => set({ weight: Number(e.currentTarget.value) })}
        >
          {WEIGHTS.map((w) => (
            <option key={w} value={w}>
              {t(`design.weights.${w}`)}
            </option>
          ))}
        </select>
      </label>
      <Color
        label={t("design.textColor")}
        value={text.color}
        onChange={(color) => set({ color })}
      />
      <div className="develop-segmented" role="radiogroup" aria-label={t("design.align")}>
        {(["left", "center", "right"] as const).map((a) => (
          <button
            key={a}
            type="button"
            role="radio"
            aria-checked={text.align === a}
            onClick={() => set({ align: a })}
          >
            {t(`design.aligns.${a}`)}
          </button>
        ))}
      </div>
      <button
        type="button"
        className="button button-small"
        onClick={() => {
          st.update({ texts: texts.filter((_, i) => i !== index) });
          st.select(null);
        }}
      >
        {t("design.removeText")}
      </button>
    </div>
  );
}

/**
 * Éditeur de livraison simple (V0.5) : la page au format du modèle, de une
 * à quatre photos, fond, marge, filet, textes et logo. Pas de calques libres
 * ni de dessin. L'export se fait à pleine résolution par le cœur.
 */
export function DesignEditor() {
  const { t } = useTranslation();
  const draft = useDesigns((s) => s.draft);
  const selected = useDesigns((s) => s.selected);
  const busy = useDesigns((s) => s.busy);
  const dirty = useDesigns((s) => s.dirty);
  const exported = useDesigns((s) => s.exported);
  const error = useDesigns((s) => s.error);
  const logoUrl = useStudioLogo((s) => s.url);
  const hasSelection = useLibrary((s) => s.selectedIds.length > 0 || s.selectedId !== null);
  const st = useDesigns.getState();

  useEffect(() => {
    void useStudioLogo.getState().load();
    void useDesigns.getState().loadFonts();
  }, []);

  const close = async () => {
    const ok =
      !useDesigns.getState().dirty ||
      (await confirmWarning(t("design.closeUnsaved"), {
        title: t("design.closeTitle"),
        okLabel: t("design.closeAnyway"),
        cancelLabel: t("design.keepEditing"),
      }));
    if (ok) st.close();
  };

  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.stopPropagation();
      void close();
    };
    window.addEventListener("keydown", key, true);
    return () => window.removeEventListener("keydown", key, true);
    // `close` lit l'état au moment de l'appel.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (!draft) return null;
  const d = draft.document;
  const text = typeof selected === "number" ? d.texts[selected] : undefined;
  const print = d.template.startsWith("print-");
  const polaroid = d.layout === "polaroid";
  // Bande photomaton : 5 × 15 cm, ou deux bandes sur un 10 × 15.
  const doubled = d.layout === "strip" && Math.round((d.width / d.height) * 3) >= 2;

  return (
    <div className="modal-backdrop">
      <div
        className="copy-dialog design-editor"
        role="dialog"
        aria-modal="true"
        aria-labelledby="design-title"
      >
        <div className="design-head">
          <h2 id="design-title" className="copy-title">
            {t("design.title", { name: t(`templates.names.${d.template}`) })}
          </h2>
          <span className="panel-note">
            {t("templates.size", { width: d.width, height: d.height })}
          </span>
          {print && (
            <IconButton
              icon={RotateCw}
              label={t("design.rotate")}
              onClick={() => st.update({ width: d.height, height: d.width })}
            />
          )}
          <span className="toolbar-spacer" />
          <IconButton icon={X} label={t("design.close")} onClick={() => void close()} />
        </div>
        <ErrorBanner error={error} onDismiss={() => useDesigns.setState({ error: null })} />
        <div className="design-body">
          <div className="design-canvas">
            <Page design={d} />
            <p className="panel-note">{t("design.hint")}</p>
          </div>
          <div className="design-side">
            <label className="deliver-field">
              <span>{t("design.name")}</span>
              <input
                type="text"
                value={draft.name}
                onChange={(e) => st.rename(e.currentTarget.value)}
              />
            </label>

            <h3 className="design-section">{t("design.photos", { count: d.photos.length })}</h3>
            <ul className="design-photos">
              {d.photos.map((p, i) => (
                <li key={`${p.item}-${i}`}>
                  <img src={previewUrl(p.item, 256) ?? undefined} alt="" />
                  <IconButton
                    icon={X}
                    label={t("design.removePhoto")}
                    disabled={d.photos.length <= 1}
                    onClick={() => st.removePhoto(i)}
                  />
                </li>
              ))}
            </ul>
            <button
              type="button"
              className="button button-small"
              disabled={d.photos.length >= maxPhotos(d.layout) || !hasSelection}
              onClick={() => {
                const lib = useLibrary.getState();
                const ids =
                  lib.selectedIds.length > 0
                    ? lib.selectedIds
                    : lib.selectedId === null
                      ? []
                      : [lib.selectedId];
                void st.addPhotos(ids);
              }}
            >
              <Plus aria-hidden="true" strokeWidth={1.75} />
              {t("design.addSelection")}
            </button>

            <h3 className="design-section">{t("design.page")}</h3>
            <Color
              label={t("design.background")}
              value={d.background}
              onChange={(background) => st.update({ background })}
            />
            {d.layout === "strip" && (
              <label className="check-row">
                <input
                  type="checkbox"
                  checked={doubled}
                  onChange={(e) => {
                    // Textes et logo gardent leur place dans la bande.
                    const k = e.currentTarget.checked ? 0.5 : 2;
                    const x = (v: number) => Math.min(Math.max(v * k, 0), 1);
                    st.update({
                      width: Math.round((d.height * (e.currentTarget.checked ? 2 : 1)) / 3),
                      texts: d.texts.map((tx) => ({ ...tx, x: x(tx.x) })),
                      logo: d.logo && { ...d.logo, x: x(d.logo.x), width: d.logo.width * k },
                    });
                  }}
                />
                {t("design.doubleStrip")}
              </label>
            )}
            {!polaroid && (
              <Slider
                label={t("design.margin")}
                value={d.margin}
                min={0}
                max={20}
                step={0.5}
                display={`${d.margin} %`}
                onChange={(margin) => st.update({ margin })}
                onReset={() => st.update({ margin: 3 })}
              />
            )}
            <label className="check-row" hidden={polaroid}>
              <input
                type="checkbox"
                checked={d.frame !== null}
                onChange={(e) =>
                  st.update({
                    frame: e.currentTarget.checked ? { width: 1, color: "#ffffff" } : null,
                  })
                }
              />
              {t("design.frame")}
            </label>
            {d.frame && (
              <>
                <Slider
                  label={t("design.frameWidth")}
                  value={d.frame.width}
                  min={0.2}
                  max={5}
                  step={0.1}
                  display={`${d.frame.width} %`}
                  onChange={(width) => d.frame && st.update({ frame: { ...d.frame, width } })}
                  onReset={() => d.frame && st.update({ frame: { ...d.frame, width: 1 } })}
                />
                <Color
                  label={t("design.frameColor")}
                  value={d.frame.color}
                  onChange={(color) => d.frame && st.update({ frame: { ...d.frame, color } })}
                />
              </>
            )}

            <h3 className="design-section">{t("design.texts")}</h3>
            <button
              type="button"
              className="button button-small"
              onClick={() => {
                st.update({
                  texts: [
                    ...d.texts,
                    {
                      ...NEW_TEXT,
                      text: t("design.newText"),
                      // Lisible sur le fond ; plus petit sur une bande étroite.
                      color: inkFor(d.background),
                      size: d.layout === "strip" ? 4 : NEW_TEXT.size,
                      x: 0.5 / columns(d.layout, d.width / d.height),
                    },
                  ],
                });
                st.select(d.texts.length);
              }}
            >
              <Type aria-hidden="true" strokeWidth={1.75} />
              {t("design.addText")}
            </button>
            {text && typeof selected === "number" ? (
              <TextControls key={selected} index={selected} text={text} />
            ) : (
              d.texts.length > 0 && <p className="panel-note">{t("design.pickText")}</p>
            )}

            <h3 className="design-section">{t("design.logo")}</h3>
            <label className="check-row">
              <input
                type="checkbox"
                checked={d.logo !== null}
                disabled={!logoUrl}
                onChange={(e) =>
                  st.update({
                    logo: e.currentTarget.checked
                      ? { x: 0.04, y: 0.04, width: 18, opacity: 90 }
                      : null,
                  })
                }
              />
              {t("design.useLogo")}
            </label>
            {!logoUrl && <p className="panel-note">{t("deliver.noStudioLogo")}</p>}
            {d.logo && (
              <>
                <Slider
                  label={t("design.logoWidth")}
                  value={d.logo.width}
                  min={5}
                  max={60}
                  step={1}
                  display={`${d.logo.width} %`}
                  onChange={(width) => d.logo && st.update({ logo: { ...d.logo, width } })}
                  onReset={() => d.logo && st.update({ logo: { ...d.logo, width: 18 } })}
                />
                <Slider
                  label={t("deliver.opacity")}
                  value={d.logo.opacity}
                  min={10}
                  max={100}
                  step={1}
                  display={`${d.logo.opacity} %`}
                  onChange={(opacity) => d.logo && st.update({ logo: { ...d.logo, opacity } })}
                  onReset={() => d.logo && st.update({ logo: { ...d.logo, opacity: 90 } })}
                />
              </>
            )}
          </div>
        </div>
        <div className="develop-crop-actions">
          {draft.id !== null && (
            <button
              type="button"
              className="button button-small"
              onClick={async () => {
                const ok = await confirmWarning(t("design.deleteText", { name: draft.name }), {
                  title: t("design.deleteTitle"),
                  okLabel: t("design.delete"),
                  cancelLabel: t("design.keepEditing"),
                });
                if (ok && draft.id !== null) await st.remove(draft.id);
              }}
            >
              {t("design.delete")}
            </button>
          )}
          <span className="panel-note design-status" role="status">
            {busy
              ? t("design.working")
              : exported
                ? t("design.exported", { file: exported })
                : dirty
                  ? t("design.unsaved")
                  : ""}
          </span>
          <span className="toolbar-spacer" />
          <button
            type="button"
            className="button button-small"
            disabled={busy || !dirty}
            onClick={() => void st.save()}
          >
            {t("design.save")}
          </button>
          <button
            type="button"
            className="button button-small button-primary"
            disabled={busy}
            onClick={() => void st.exportDraft()}
          >
            {t("design.export")}
          </button>
        </div>
      </div>
    </div>
  );
}
