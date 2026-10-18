import { ImageOff } from "lucide-react";
import { useTranslation } from "react-i18next";
import { previewUrl } from "../api/previews";
import { useLibrary } from "../store/library";
import {
  formatAperture,
  formatCaptureTime,
  formatCompensation,
  formatFocal,
  formatIso,
  formatShutter,
} from "../ui/format";
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
  const total = useLibrary((s) => s.total);
  const showAlbum = useLibrary((s) => s.showAlbum);
  const dateFormat = new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium" });

  return (
    <>
      {navigator && <NavigatorSection scope={scope} />}
      <PanelSection id={`${scope}.catalog`} title={t("catalog.title")}>
        <ListRow
          label={t("catalog.allPhotos")}
          count={total}
          selected={albumId === null}
          onClick={() => void showAlbum(null)}
        />
        <ListRow label={t("catalog.trash")} count={0} />
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
              selected={albumId === a.id}
              onClick={() => void showAlbum(a.id)}
            />
          ))
        )}
      </PanelSection>
      <PanelSection id={`${scope}.collections`} title={t("catalog.collections")}>
        <PanelNote>{t("catalog.noCollections")}</PanelNote>
      </PanelSection>
    </>
  );
}

/** Panneau droit : histogramme et fiche d'informations de la photo sélectionnée. */
export function PhotoInfoPanel({ scope }: { scope: string }) {
  const { t, i18n } = useTranslation();
  const d = useLibrary((s) => s.details);
  const lng = i18n.language;

  return (
    <>
      <PanelSection id={`${scope}.histogram`} title={t("info.histogram")}>
        <div className="histogram" aria-hidden="true" />
        <div className="histogram-exif">
          <span>{formatIso(d?.iso ?? null) ?? "ISO —"}</span>
          <span>{formatFocal(d?.focalLength ?? null, lng) ?? "— mm"}</span>
          <span>{formatAperture(d?.aperture ?? null, lng) ?? "f / —"}</span>
          <span>{formatShutter(d?.shutter ?? null) ?? "— s"}</span>
        </div>
      </PanelSection>
      <PanelSection id={`${scope}.metadata`} title={t("info.metadata")}>
        <FieldRow label={t("info.fileName")} value={d?.fileName} />
        <FieldRow label={t("info.originalName")} value={d?.originalName} />
        <FieldRow
          label={t("info.captureTime")}
          value={formatCaptureTime(d?.capturedAt ?? null, lng)}
        />
        <FieldRow
          label={t("info.dimensions")}
          value={d?.width && d.height ? `${d.width} × ${d.height}` : undefined}
        />
        <FieldRow label={t("info.camera")} value={d?.camera} />
        <FieldRow label={t("info.lens")} value={d?.lens} />
        <FieldRow
          label={t("info.exposureCompensation")}
          value={formatCompensation(d?.exposureCompensation ?? null, lng)}
        />
      </PanelSection>
      <PanelSection id={`${scope}.keywords`} title={t("info.keywords")}>
        {d && d.keywords.length > 0 ? (
          <p className="keyword-list">{d.keywords.join(", ")}</p>
        ) : (
          <PanelNote>{d ? t("info.noKeywords") : t("info.noSelection")}</PanelNote>
        )}
      </PanelSection>
    </>
  );
}
