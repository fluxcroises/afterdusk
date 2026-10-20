import { Images, Share } from "lucide-react";
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
import { EmptyState } from "../components/EmptyState";
import { ErrorBanner } from "../components/ErrorBanner";
import { MediaGrid } from "../components/MediaGrid";
import { ScreenLayout } from "../components/ScreenLayout";
import { useDeliver } from "../store/deliver";
import { useLibrary } from "../store/library";

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

  useEffect(() => {
    void useDeliver.getState().load();
  }, []);

  return (
    <ScreenLayout
      left={<CatalogPanel scope="deliver" />}
      right={
        <>
          <ExportPresetsSection />
          <WatermarkSection />
          <DestinationSection />
        </>
      }
      rightFooter={<ExportButton />}
      toolbar={
        <>
          <span className="toolbar-status">{t("deliver.selection", { count: selected })}</span>
          <span className="toolbar-spacer" />
          <ExportStatus />
        </>
      }
    >
      <ErrorBanner error={error} onDismiss={() => useDeliver.setState({ error: null })} />
      {count > 0 ? (
        <MediaGrid />
      ) : (
        <EmptyState icon={Images} title={t("deliver.emptyTitle")}>
          <p>{t("deliver.emptyText")}</p>
        </EmptyState>
      )}
      {count > 0 && selected === 0 && (
        <p className="deliver-hint">
          <Share aria-hidden="true" strokeWidth={1.75} />
          {t("deliver.selectHint")}
        </p>
      )}
    </ScreenLayout>
  );
}
