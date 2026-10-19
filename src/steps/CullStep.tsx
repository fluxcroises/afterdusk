import { Images } from "lucide-react";
import { useTranslation } from "react-i18next";
import { MediaGrid } from "../components/MediaGrid";
import { useLibrary } from "../store/library";
import { CatalogPanel, PhotoInfoPanel } from "../components/CatalogPanels";
import { ErrorBanner } from "../components/ErrorBanner";
import { EmptyState, Kbd } from "../components/EmptyState";
import { Filmstrip } from "../components/Filmstrip";
import { GridToolbar } from "../components/GridToolbar";
import { ScreenLayout } from "../components/ScreenLayout";

export function CullStep() {
  const { t } = useTranslation();
  const count = useLibrary((s) => s.media.length);
  const error = useLibrary((s) => s.error);
  return (
    <ScreenLayout
      left={<CatalogPanel scope="cull" navigator />}
      right={<PhotoInfoPanel scope="cull" />}
      toolbar={<GridToolbar count={count} culling />}
      filmstrip={<Filmstrip />}
    >
      <ErrorBanner error={error} onDismiss={() => useLibrary.setState({ error: null })} />
      {count > 0 ? (
        <MediaGrid />
      ) : (
        <EmptyState icon={Images} title={t("cull.emptyTitle")}>
          <p>{t("cull.emptyText")}</p>
          <p className="shortcut-hint">
            <Kbd>P</Kbd> {t("cull.pick")} · <Kbd>X</Kbd> {t("cull.reject")} · <Kbd>1</Kbd>–
            <Kbd>5</Kbd> {t("cull.stars")} · <Kbd>6</Kbd>–<Kbd>9</Kbd> {t("cull.colors")}
          </p>
        </EmptyState>
      )}
    </ScreenLayout>
  );
}
