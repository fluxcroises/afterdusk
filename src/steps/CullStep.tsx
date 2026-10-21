import { Images, SearchX, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Compare } from "../components/Compare";
import { Loupe } from "../components/Loupe";
import { MediaGrid } from "../components/MediaGrid";
import { isFiltering, useLibrary } from "../store/library";
import { CatalogPanel, PhotoInfoPanel } from "../components/CatalogPanels";
import { ErrorBanner } from "../components/ErrorBanner";
import { FilterBar } from "../components/FilterBar";
import { TrashBar, TrashNotice } from "../components/TrashBanners";
import { EmptyState, Kbd } from "../components/EmptyState";
import { Filmstrip } from "../components/Filmstrip";
import { GridToolbar } from "../components/GridToolbar";
import { ScreenLayout } from "../components/ScreenLayout";

export function CullStep() {
  const { t } = useTranslation();
  const count = useLibrary((s) => s.media.length);
  const error = useLibrary((s) => s.error);
  const view = useLibrary((s) => s.view);
  const trash = useLibrary((s) => s.trash);
  const filtering = useLibrary((s) => isFiltering(s.filter));
  return (
    <ScreenLayout
      left={<CatalogPanel scope="cull" navigator />}
      right={<PhotoInfoPanel scope="cull" />}
      toolbar={<GridToolbar count={count} culling />}
      header={<FilterBar />}
      filmstrip={<Filmstrip />}
    >
      <ErrorBanner error={error} onDismiss={() => useLibrary.setState({ error: null })} />
      <TrashNotice />
      <TrashBar />
      {trash && count === 0 ? (
        <EmptyState icon={Trash2} title={t("trash.emptyTitle")} />
      ) : count === 0 && filtering ? (
        <EmptyState icon={SearchX} title={t("filter.noMatchTitle")}>
          <button
            type="button"
            className="button button-small"
            onClick={() => void useLibrary.getState().resetFilter()}
          >
            {t("filter.reset")}
          </button>
        </EmptyState>
      ) : count > 0 ? (
        view === "loupe" ? (
          <Loupe />
        ) : view === "compare" ? (
          <Compare />
        ) : (
          <MediaGrid />
        )
      ) : (
        <EmptyState icon={Images} title={t("cull.emptyTitle")}>
          <p className="shortcut-hint">
            <Kbd>P</Kbd> {t("cull.pick")} · <Kbd>X</Kbd> {t("cull.reject")} · <Kbd>1</Kbd>–
            <Kbd>5</Kbd> {t("cull.stars")} · <Kbd>6</Kbd>–<Kbd>9</Kbd> {t("cull.colors")}
          </p>
        </EmptyState>
      )}
    </ScreenLayout>
  );
}
