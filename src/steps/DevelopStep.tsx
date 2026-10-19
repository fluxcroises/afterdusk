import { ImageOff, Images, SlidersHorizontal } from "lucide-react";
import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { CatalogPanel } from "../components/CatalogPanels";
import { EmptyState } from "../components/EmptyState";
import { Filmstrip } from "../components/Filmstrip";
import { FieldRow, PanelSection } from "../components/PanelSection";
import { ScreenLayout } from "../components/ScreenLayout";
import { EXPOSURE_RANGE, useDevelop } from "../store/develop";
import { useLibrary } from "../store/library";

/** Image rendue par le cœur, dessinée telle quelle (aucun calcul de couleur ici). */
function DevelopCanvas() {
  const { t } = useTranslation();
  const frame = useDevelop((s) => s.frame);
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const canvas = ref.current;
    const context = canvas?.getContext("2d");
    if (!canvas || !context || !frame) return;
    canvas.width = frame.width;
    canvas.height = frame.height;
    context.putImageData(new ImageData(frame.pixels, frame.width, frame.height), 0, 0);
  }, [frame]);
  if (!frame) return null;
  return <canvas ref={ref} className="develop-canvas" aria-label={t("develop.image")} />;
}

function formatEv(value: number, language: string) {
  return new Intl.NumberFormat(language, {
    signDisplay: "exceptZero",
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  }).format(value);
}

function SettingsPanel() {
  const { t, i18n } = useTranslation();
  const settings = useDevelop((s) => s.settings);
  const setExposure = useDevelop((s) => s.setExposure);
  const exposure = settings?.light?.exposure ?? 0;
  return (
    <>
      <PanelSection id="develop.light" title={t("develop.light")}>
        <label className="develop-slider">
          <span className="develop-slider-head">
            <span>{t("develop.exposure")}</span>
            <output>{t("develop.ev", { value: formatEv(exposure, i18n.language) })}</output>
          </span>
          <input
            type="range"
            min={EXPOSURE_RANGE.min}
            max={EXPOSURE_RANGE.max}
            step={EXPOSURE_RANGE.step}
            value={exposure}
            disabled={!settings}
            title={t("develop.resetHint")}
            onChange={(e) => setExposure(Number(e.currentTarget.value))}
            onDoubleClick={() => setExposure(0)}
          />
        </label>
      </PanelSection>
      <PanelSection id="develop.color" title={t("develop.color")}>
        <FieldRow label={t("develop.whiteBalance")} value={t("develop.asShot")} />
      </PanelSection>
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
