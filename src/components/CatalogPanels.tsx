import { invoke } from "@tauri-apps/api/core";
import { HistogramCanvas } from "./DevelopPanels";
import { AlbumClientSection, ClientsSection } from "./ClientsPanels";
import { ImageOff } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { previewUrl } from "../api/previews";
import { useLibrary } from "../store/library";
import { formatCaptureTime, formatCompensation, formatDegrees, placeName } from "../ui/format";
import { CollectionsSection } from "./CollectionsSection";
import { ExifLine } from "./ExifLine";
import { FieldRow, ListRow, PanelNote, PanelSection } from "./PanelSection";
import { Thumbnail } from "./Thumbnail";

/** Aperçu réduit de la photo affichée, avec les niveaux de zoom. */
export function NavigatorSection({ scope }: { scope: string }) {
  const { t } = useTranslation();
  const selected = useLibrary((s) => s.media.find((m) => m.id === s.selectedId));
  const version = useLibrary((s) => (s.selectedId === null ? 0 : (s.versions[s.selectedId] ?? 0)));
  return (
    <PanelSection
      id={`${scope}.navigator`}
      title={t("navigator.title")}
      aside={
        <span className="panel-section-aside">
          <span>{t("navigator.fit")}</span>
          <span>{t("navigator.full")}</span>
        </span>
      }
    >
      <div className="navigator-preview">
        {selected?.hasPreview ? (
          <Thumbnail src={previewUrl(selected.id, 1024, version)} />
        ) : (
          <>
            <ImageOff aria-hidden="true" strokeWidth={1.25} />
            <span className="visually-hidden">{t("navigator.empty")}</span>
          </>
        )}
      </div>
    </PanelSection>
  );
}

/** Panneau gauche partagé par Trier et Bibliothèque : catalogue, albums, collections. */
export function CatalogPanel({ scope, navigator }: { scope: string; navigator?: boolean }) {
  const { t, i18n } = useTranslation();
  const albums = useLibrary((s) => s.albums);
  const albumId = useLibrary((s) => s.albumId);
  const collectionId = useLibrary((s) => s.collectionId);
  const total = useLibrary((s) => s.total);
  const trash = useLibrary((s) => s.trash);
  const trashCount = useLibrary((s) => s.trashCount);
  const showAlbum = useLibrary((s) => s.showAlbum);
  const showTrash = useLibrary((s) => s.showTrash);
  const dateFormat = new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium" });

  return (
    <>
      {navigator && <NavigatorSection scope={scope} />}
      <PanelSection id={`${scope}.catalog`} title={t("catalog.title")}>
        <ListRow
          label={t("catalog.allPhotos")}
          count={total}
          selected={!trash && albumId === null && collectionId === null}
          onClick={() => void showAlbum(null)}
        />
        <ListRow
          label={t("catalog.trash")}
          count={trashCount}
          selected={trash}
          onClick={() => void showTrash()}
        />
      </PanelSection>
      <PanelSection id={`${scope}.albums`} title={t("catalog.albums")}>
        {albums.length === 0 ? (
          <PanelNote>{t("catalog.noAlbums")}</PanelNote>
        ) : (
          albums.map((a) => (
            <ListRow
              key={a.id}
              label={a.name}
              hint={
                a.startDate ? dateFormat.format(new Date(`${a.startDate}T12:00:00`)) : undefined
              }
              count={a.count}
              selected={!trash && albumId === a.id}
              onClick={() => void showAlbum(a.id)}
            />
          ))
        )}
      </PanelSection>
      <CollectionsSection scope={scope} />
      <ClientsSection scope={scope} />
    </>
  );
}

/** Histogramme de l'aperçu de la photo sélectionnée (retouche comprise). */
function PreviewHistogram() {
  const item = useLibrary((s) => s.selectedId);
  const version = useLibrary((s) => (s.selectedId === null ? 0 : (s.versions[s.selectedId] ?? 0)));
  const hasPreview = useLibrary(
    (s) => s.media.find((m) => m.id === s.selectedId)?.hasPreview ?? false,
  );
  // Résultat rangé avec la photo et la version demandées : une autre photo
  // n'affiche pas l'histogramme de la précédente.
  const key = `${item}-${version}`;
  const [loaded, setLoaded] = useState<{ key: string; bins: Uint32Array | null } | null>(null);
  useEffect(() => {
    if (item === null || !hasPreview) return;
    let current = true;
    invoke<number[] | null>("media_histogram", { mediaId: item })
      .then((bins) => current && setLoaded({ key, bins: bins ? Uint32Array.from(bins) : null }))
      .catch(() => current && setLoaded({ key, bins: null }));
    return () => {
      current = false;
    };
  }, [item, hasPreview, key]);
  const histogram = loaded?.key === key && hasPreview ? loaded.bins : null;
  return <HistogramCanvas histogram={histogram} />;
}

/** Panneau droit : histogramme et fiche d'informations de la photo sélectionnée. */
export function PhotoInfoPanel({ scope }: { scope: string }) {
  const { t, i18n } = useTranslation();
  const d = useLibrary((s) => s.details);
  const lng = i18n.language;

  return (
    <>
      <AlbumClientSection scope={scope} />
      <PanelSection id={`${scope}.histogram`} title={t("info.histogram")}>
        <div className="histogram" aria-hidden="true">
          <PreviewHistogram />
        </div>
        <ExifLine exposure={d} />
      </PanelSection>
      <PanelSection id={`${scope}.metadata`} title={t("info.metadata")}>
        {d ? (
          <>
            <FieldRow label={t("info.fileName")} value={d.fileName} />
            {d.stackFiles.length > 1 && (
              <FieldRow label={t("info.stackFiles")} value={d.stackFiles.join(" + ")} />
            )}
            <FieldRow label={t("info.originalName")} value={d.originalName} />
            <FieldRow label={t("info.captureTime")} value={formatCaptureTime(d.capturedAt, lng)} />
            {d.capturedAtCamera && (
              <FieldRow
                label={t("info.cameraTime")}
                value={formatCaptureTime(d.capturedAtCamera, lng)}
              />
            )}
            <FieldRow
              label={t("info.dimensions")}
              value={d.width && d.height ? `${d.width} × ${d.height}` : undefined}
            />
            <FieldRow label={t("info.camera")} value={d.camera} />
            <FieldRow label={t("info.lens")} value={d.lens} />
            <FieldRow
              label={t("info.exposureCompensation")}
              value={formatCompensation(d.exposureCompensation, lng)}
            />
            <FieldRow label={t("info.place")} value={placeName(d.city, d.country, lng)} />
            <FieldRow
              label={t("info.coordinates")}
              value={
                d.latitude !== null && d.longitude !== null
                  ? `${t("info.placeValue", {
                      lat: formatDegrees(d.latitude, lng),
                      latRef: t(d.latitude < 0 ? "info.hemispheres.s" : "info.hemispheres.n"),
                      lon: formatDegrees(d.longitude, lng),
                      lonRef: t(d.longitude < 0 ? "info.hemispheres.w" : "info.hemispheres.e"),
                    })}`
                  : undefined
              }
            />
            {d.locationSource && (
              <FieldRow
                label={t("info.placeSource")}
                value={t(`info.placeSources.${d.locationSource}`)}
              />
            )}
          </>
        ) : (
          <PanelNote>{t("info.noSelection")}</PanelNote>
        )}
      </PanelSection>
      {d && (
        <PanelSection id={`${scope}.keywords`} title={t("info.keywords")}>
          {d.keywords.length > 0 ? (
            <p className="keyword-list">{d.keywords.join(", ")}</p>
          ) : (
            <PanelNote>{t("info.noKeywords")}</PanelNote>
          )}
        </PanelSection>
      )}
    </>
  );
}
