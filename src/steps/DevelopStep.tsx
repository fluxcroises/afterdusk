import { ImageOff, Images, SlidersHorizontal } from "lucide-react";
import { type MouseEvent, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { CatalogPanel } from "../components/CatalogPanels";
import {
  ColorSection,
  CompareControls,
  HistogramSection,
  HistorySection,
  LightSection,
} from "../components/DevelopPanels";
import { CurveSection, EffectsSection, HslSection } from "../components/DevelopAdvanced";
import { SnapshotsSection, VersionsSection } from "../components/DevelopVersions";
import { EmptyState } from "../components/EmptyState";
import { Filmstrip } from "../components/Filmstrip";
import { ScreenLayout } from "../components/ScreenLayout";
import { type DevelopFrame, useDevelop } from "../store/develop";
import { useLibrary } from "../store/library";

/** Saisie de texte en cours (un curseur de réglage n'en est pas une). */
function isTyping(target: EventTarget | null): boolean {
  if (target instanceof HTMLInputElement) return target.type !== "range";
  return (
    target instanceof HTMLElement &&
    (target.isContentEditable || ["TEXTAREA", "SELECT"].includes(target.tagName))
  );
}

/** Image rendue par le cœur, dessinée telle quelle (aucun calcul de couleur ici). */
function DevelopCanvas() {
  const { t } = useTranslation();
  const frame = useDevelop((s) => s.frame);
  const before = useDevelop((s) => s.beforeFrame);
  const compare = useDevelop((s) => s.compare);
  const picking = useDevelop((s) => s.picking);
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const canvas = ref.current;
    const context = canvas?.getContext("2d");
    if (!canvas || !context || !frame) return;
    canvas.width = frame.width;
    canvas.height = frame.height;
    const image = (f: DevelopFrame) => new ImageData(f.pixels, f.width, f.height);
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
  }, [frame, before, compare]);

  // Pipette : le point cliqué, en pixels de l'aperçu rendu.
  const pick = (e: MouseEvent<HTMLCanvasElement>) => {
    if (!picking || !frame) return;
    const rect = e.currentTarget.getBoundingClientRect();
    const x = ((e.clientX - rect.left) / rect.width) * frame.width;
    const y = ((e.clientY - rect.top) / rect.height) * frame.height;
    void useDevelop.getState().pickWhite(x, y);
  };

  if (!frame) return null;
  return (
    <canvas
      ref={ref}
      className="develop-canvas"
      data-picking={picking || undefined}
      aria-label={t("develop.image")}
      onClick={pick}
    />
  );
}

function SettingsPanel() {
  return (
    <>
      <HistogramSection />
      <LightSection />
      <ColorSection />
      <CurveSection />
      <HslSection />
      <EffectsSection />
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
  if (picking) return <span className="toolbar-status">{t("develop.pickHint")}</span>;
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

  useEffect(() => {
    if (selectedId !== null) void useDevelop.getState().open(selectedId);
  }, [selectedId]);
  // Quitter le Retoucher enregistre le dernier réglage et libère l'image.
  useEffect(() => () => void useDevelop.getState().close(), []);
  // « \ » : l'original ou la retouche, comme dans Lightroom (AltGr+8 en AZERTY).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "\\" || isTyping(e.target)) return;
      const { compare, setCompare } = useDevelop.getState();
      setCompare(compare === "before" ? "off" : "before");
      e.preventDefault();
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
        <div className="develop-view" aria-busy={loading}>
          {loading && <p className="develop-loading">{t("develop.loading")}</p>}
          <DevelopCanvas />
        </div>
      )}
    </ScreenLayout>
  );
}
