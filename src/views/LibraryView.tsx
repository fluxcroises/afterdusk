import { Library } from "lucide-react";
import { useTranslation } from "react-i18next";
import { MediaGrid } from "../components/MediaGrid";
import { useLibrary } from "../store/library";
import { CatalogPanel, PhotoInfoPanel } from "../components/CatalogPanels";
import { EmptyState } from "../components/EmptyState";
import { GridToolbar } from "../components/GridToolbar";
import { ScreenLayout } from "../components/ScreenLayout";

export function LibraryView() {
  const { t } = useTranslation();
  const count = useLibrary((s) => s.media.length);
  return (
    <ScreenLayout
      left={<CatalogPanel scope="library" />}
      right={<PhotoInfoPanel scope="library" />}
      toolbar={<GridToolbar count={count} />}
    >
      {count > 0 ? (
        <MediaGrid />
      ) : (
        <EmptyState icon={Library} title={t("library.emptyTitle")}>
          {t("library.emptyText")}
        </EmptyState>
      )}
    </ScreenLayout>
  );
}
