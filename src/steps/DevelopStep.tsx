import { ImageOff, Images, SlidersHorizontal } from "lucide-react";
import { type MouseEvent, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { CatalogPanel } from "../components/CatalogPanels";
import { ColorSection, HistogramSection, LightSection } from "../components/DevelopPanels";
import { EmptyState } from "../components/EmptyState";
import { Filmstrip } from "../components/Filmstrip";
import { ScreenLayout } from "../components/ScreenLayout";
import { useDevelop } from "../store/develop";
import { useLibrary } from "../store/library";

/** Image rendue par le cœur, dessinée telle quelle (aucun calcul de couleur ici). */
function DevelopCanvas() {
  const { t } = useTranslation();
  const frame = useDevelop((s) => s.frame);
  const picking = useDevelop((s) => s.picking);
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const canvas = ref.current;
    const context = canvas?.getContext("2d");
    if (!canvas || !context || !frame) return;
    canvas.width = frame.width;
    canvas.height = frame.height;
    context.putImageData(new ImageData(frame.pixels, frame.width, frame.height), 0, 0);
  }, [frame]);

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
  if (picking) return <span className="toolbar-status">{t("develop.pickHint")}</span>;
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

  return (
    <ScreenLayout
      left={<CatalogPanel scope="develop" />}
      right={<SettingsPanel />}
      toolbar={
        <>
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
