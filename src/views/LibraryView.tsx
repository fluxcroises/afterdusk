import { Library } from "lucide-react";
import { useTranslation } from "react-i18next";
import { CatalogPanel, PhotoInfoPanel } from "../components/CatalogPanels";
import { EmptyState } from "../components/EmptyState";
import { GridToolbar } from "../components/GridToolbar";
import { ScreenLayout } from "../components/ScreenLayout";

export function LibraryView() {
  const { t } = useTranslation();
  return (
    <ScreenLayout
      left={<CatalogPanel scope="library" />}
      right={<PhotoInfoPanel scope="library" />}
      toolbar={<GridToolbar count={0} />}
    >
      <EmptyState icon={Library} title={t("library.emptyTitle")}>
        {t("library.emptyText")}
      </EmptyState>
    </ScreenLayout>
  );
}
