import { Library } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Compare } from "../components/Compare";
import { Loupe } from "../components/Loupe";
import { MediaGrid } from "../components/MediaGrid";
import { useLibrary } from "../store/library";
import { CatalogPanel, PhotoInfoPanel } from "../components/CatalogPanels";
import { ErrorBanner } from "../components/ErrorBanner";
import { EmptyState } from "../components/EmptyState";
import { GridToolbar } from "../components/GridToolbar";
import { ScreenLayout } from "../components/ScreenLayout";

export function LibraryView() {
  const { t } = useTranslation();
  const count = useLibrary((s) => s.media.length);
  const error = useLibrary((s) => s.error);
  const view = useLibrary((s) => s.view);
  return (
    <ScreenLayout
      left={<CatalogPanel scope="library" />}
      right={<PhotoInfoPanel scope="library" />}
      toolbar={<GridToolbar count={count} />}
    >
      <ErrorBanner error={error} onDismiss={() => useLibrary.setState({ error: null })} />
      {count > 0 ? (
        view === "loupe" ? (
          <Loupe />
        ) : view === "compare" ? (
          <Compare />
        ) : (
          <MediaGrid />
        )
      ) : (
        <EmptyState icon={Library} title={t("library.emptyTitle")}>
          {t("library.emptyText")}
        </EmptyState>
      )}
    </ScreenLayout>
  );
}
