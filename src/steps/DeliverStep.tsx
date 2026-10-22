import { useGallery } from "../store/gallery";
import { useContactSheet } from "../store/contactSheet";
import { Images } from "lucide-react";
import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { CatalogPanel } from "../components/CatalogPanels";
import {
  DestinationSection,
  ExportButton,
  ExportPresetsSection,
  ExportStatus,
  WatermarkSection,
} from "../components/DeliverPanels";
import { ResultPreview } from "../components/DeliverPreview";
import { TemplatesView } from "../components/DeliveryTemplates";
import { EmptyState } from "../components/EmptyState";
import { ErrorBanner } from "../components/ErrorBanner";
import { MediaGrid } from "../components/MediaGrid";
import { ScreenLayout } from "../components/ScreenLayout";
import { useDeliver } from "../store/deliver";
import { useLibrary } from "../store/library";
import { useTemplates } from "../store/templates";

/**
 * Livrer : la sélection au centre, les presets d'export cochés à droite
 * avec leurs options, le filigrane et la destination ; progression et bilan
 * dans la barre d'outils.
 */
export function DeliverStep() {
  const { t } = useTranslation();
  const count = useLibrary((s) => s.media.length);
  const selected = useLibrary((s) =>
    s.selectedIds.length > 1 ? s.selectedIds.length : s.selectedId === null ? 0 : 1,
  );
  const error = useDeliver((s) => s.error);
  const view = useTemplates((s) => s.view);
  const templatesError = useTemplates((s) => s.error);
  const chosenTemplates = useTemplates((s) => s.chosen.length);

  useEffect(() => {
    void useDeliver.getState().load();
    // Modèles cochés : leur liste sert au bouton d'export, même sans la vue.
    void useTemplates.getState().load(null);
  }, []);

  return (
    <ScreenLayout
      left={<CatalogPanel scope="deliver" />}
      right={
        <>
          <ResultPreview />
          <ExportPresetsSection />
          <WatermarkSection />
          <DestinationSection />
        </>
      }
      rightFooter={<ExportButton />}
      toolbar={
        <>
          <div
            className="develop-segmented deliver-view"
            role="radiogroup"
            aria-label={t("templates.viewLabel")}
          >
            {(["photos", "templates"] as const).map((v) => (
              <button
                key={v}
                type="button"
                role="radio"
                aria-checked={view === v}
                onClick={() => useTemplates.getState().setView(v)}
              >
                {t(v === "photos" ? "templates.photosTab" : "templates.tab")}
              </button>
            ))}
          </div>
          <span className="toolbar-status">{t("deliver.selection", { count: selected })}</span>
          {chosenTemplates > 0 && (
            <span className="toolbar-status">
              {t("templates.chosen", { count: chosenTemplates })}
            </span>
          )}
          <span className="toolbar-spacer" />
          <ExportStatus />
          <button
            type="button"
            className="button button-small"
            disabled={count === 0}
            onClick={() => useContactSheet.getState().show()}
          >
            {t("contactSheet.open")}
          </button>
          <button
            type="button"
            className="button button-small"
            disabled={count === 0}
            onClick={() => useGallery.getState().show()}
          >
            {t("gallery.open")}
          </button>
          <button
            type="button"
            className="button button-small"
            onClick={() => useGallery.getState().showImport()}
          >
            {t("gallery.import.open")}
          </button>
        </>
      }
    >
      <ErrorBanner error={error} onDismiss={() => useDeliver.setState({ error: null })} />
      <ErrorBanner
        error={templatesError}
        onDismiss={() => useTemplates.setState({ error: null })}
      />
      {count === 0 ? (
        <EmptyState icon={Images} title={t("deliver.emptyTitle")} />
      ) : view === "templates" ? (
        <TemplatesView />
      ) : (
        <MediaGrid />
      )}
    </ScreenLayout>
  );
}
