import { MapPin, Pointer, SquareDashed } from "lucide-react";
import { lazy, Suspense, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { previewUrl } from "../api/previews";
import { ErrorBanner } from "../components/ErrorBanner";
import { FieldRow, ListRow, PanelNote, PanelSection } from "../components/PanelSection";
import { ScreenLayout } from "../components/ScreenLayout";
import { Thumbnail } from "../components/Thumbnail";
import { useLibrary } from "../store/library";
import { useMap } from "../store/map";
import { useNavigation } from "../store/navigation";
import { albumDates, placeName } from "../ui/format";
import { inArea, toMicro } from "../ui/geo";

// MapLibre (environ 1 Mo) n'est chargé qu'à l'ouverture de la Carte.
const WorldMap = lazy(() =>
  import("../components/WorldMap").then((m) => ({ default: m.WorldMap })),
);

/** Ouvre un album dans la Bibliothèque. */
async function openAlbum(albumId: number) {
  useNavigation.getState().goTo("library");
  await useLibrary.getState().showAlbum(albumId);
}

function AlbumList() {
  const { t, i18n } = useTranslation();
  const albums = useMap((s) => s.albums);
  const selected = useMap((s) => s.selectedAlbumId);
  return (
    <PanelSection id="map.albums" title={t("map.albums")}>
      {albums.length === 0 ? (
        <PanelNote>{t("map.noAlbum")}</PanelNote>
      ) : (
        albums.map((a) => (
          <ListRow
            key={a.albumId}
            label={a.name}
            hint={albumDates(a, i18n.language) ?? undefined}
            count={a.located}
            selected={a.albumId === selected}
            onClick={() => void useMap.getState().selectAlbum(a.albumId)}
          />
        ))
      )}
    </PanelSection>
  );
}

function AlbumPreview() {
  const { t, i18n } = useTranslation();
  const album = useMap((s) => s.albums.find((a) => a.albumId === s.selectedAlbumId));
  const tracks = useMap((s) => s.tracks.length);
  return (
    <PanelSection id="map.album" title={t("map.album")}>
      {album ? (
        <div className="map-preview">
          <span className="map-preview-cover">
            <Thumbnail src={previewUrl(album.coverMediaId, 1024)} />
          </span>
          <FieldRow label={t("map.name")} value={album.name} />
          <FieldRow label={t("map.dates")} value={albumDates(album, i18n.language) ?? undefined} />
          <FieldRow
            label={t("info.place")}
            value={placeName(album.city, album.country, i18n.language)}
          />
          <FieldRow
            label={t("map.located")}
            value={t("map.locatedOf", { count: album.located, total: album.total })}
          />
          {tracks > 0 && <FieldRow label={t("map.tracks")} value={String(tracks)} />}
          <button
            type="button"
            className="button button-secondary"
            onClick={() => void openAlbum(album.albumId)}
          >
            {t("map.openAlbum")}
          </button>
        </div>
      ) : (
        <PanelNote>{t("map.noSelection")}</PanelNote>
      )}
    </PanelSection>
  );
}

function AreaSection() {
  const { t } = useTranslation();
  const area = useMap((s) => s.area);
  const count = useMap((s) => {
    const a = s.area;
    return a ? s.media.filter((p) => inArea(p.longitude, p.latitude, a)).length : 0;
  });
  if (!area) return null;
  return (
    <PanelSection id="map.area" title={t("map.area")}>
      <FieldRow label={t("map.areaPhotos")} value={String(count)} />
      <div className="map-area-actions">
        <button
          type="button"
          className="button button-secondary"
          disabled={count === 0}
          onClick={() => {
            const library = useLibrary.getState();
            useNavigation.getState().goTo("library");
            void library.showAlbum(null).then(() => library.setFilter({ area: toMicro(area) }));
          }}
        >
          {t("map.showArea", { count })}
        </button>
        <button
          type="button"
          className="button button-small"
          onClick={() => useMap.getState().setArea(null)}
        >
          {t("map.clearArea")}
        </button>
      </div>
    </PanelSection>
  );
}

function MapToolbar() {
  const { t } = useTranslation();
  const mode = useMap((s) => s.mode);
  const albums = useMap((s) => s.albums.length);
  const photos = useMap((s) => s.media.length);
  const placed = useMap((s) => s.placed);
  const selection = useLibrary((s) =>
    s.selectedIds.length > 0 ? s.selectedIds.length : s.selectedId !== null ? 1 : 0,
  );
  const st = useMap.getState();
  const status =
    mode === "place"
      ? t("map.placing", { count: selection })
      : mode === "area"
        ? t("map.drawing")
        : placed !== null
          ? t("locate.report.located", { count: placed })
          : `${t("map.albumCount", { count: albums })} · ${t("map.locatedCount", { count: photos })}`;
  return (
    <>
      <button
        type="button"
        className="button button-small map-tool"
        aria-pressed={mode === "area"}
        onClick={() => st.setMode("area")}
      >
        <SquareDashed aria-hidden="true" />
        {t("map.drawArea")}
      </button>
      <button
        type="button"
        className="button button-small map-tool"
        aria-pressed={mode === "place"}
        disabled={selection === 0}
        onClick={() => st.setMode("place")}
      >
        {mode === "place" ? <Pointer aria-hidden="true" /> : <MapPin aria-hidden="true" />}
        {t("map.place", { count: selection })}
      </button>
      <span className="toolbar-spacer" />
      <span className="toolbar-status" title={status}>
        {status}
      </span>
    </>
  );
}

export function MapView() {
  const libraryId = useLibrary((s) => s.libraryId);
  const error = useMap((s) => s.error);

  useEffect(() => {
    if (libraryId !== null) void useMap.getState().load(libraryId);
  }, [libraryId]);

  useEffect(() => {
    const escape = (e: KeyboardEvent) => {
      if (e.key === "Escape" && useMap.getState().mode !== "browse") {
        useMap.setState({ mode: "browse" });
      }
    };
    window.addEventListener("keydown", escape);
    return () => window.removeEventListener("keydown", escape);
  }, []);

  return (
    <ScreenLayout
      left={<AlbumList />}
      right={
        <>
          <AlbumPreview />
          <AreaSection />
        </>
      }
      toolbar={<MapToolbar />}
    >
      <ErrorBanner error={error} onDismiss={() => useMap.setState({ error: null })} />
      <Suspense fallback={null}>
        <WorldMap onOpenAlbum={(id) => void openAlbum(id)} />
      </Suspense>
    </ScreenLayout>
  );
}
