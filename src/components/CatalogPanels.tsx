import { ImageOff } from "lucide-react";
import { useTranslation } from "react-i18next";
import { FieldRow, ListRow, PanelNote, PanelSection } from "./PanelSection";

/** Aperçu réduit de la photo affichée, avec les niveaux de zoom. */
export function NavigatorSection({ scope }: { scope: string }) {
  const { t } = useTranslation();
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
        <ImageOff aria-hidden="true" strokeWidth={1.25} />
        <span className="visually-hidden">{t("navigator.empty")}</span>
      </div>
    </PanelSection>
  );
}

/** Panneau gauche partagé par Trier et Bibliothèque : catalogue, albums, collections. */
export function CatalogPanel({ scope, navigator }: { scope: string; navigator?: boolean }) {
  const { t } = useTranslation();
  return (
    <>
      {navigator && <NavigatorSection scope={scope} />}
      <PanelSection id={`${scope}.catalog`} title={t("catalog.title")}>
        <ListRow label={t("catalog.allPhotos")} count={0} selected />
        <ListRow label={t("catalog.lastImport")} count={0} />
        <ListRow label={t("catalog.trash")} count={0} />
      </PanelSection>
      <PanelSection id={`${scope}.albums`} title={t("catalog.albums")}>
        <PanelNote>{t("catalog.noAlbums")}</PanelNote>
      </PanelSection>
      <PanelSection id={`${scope}.collections`} title={t("catalog.collections")}>
        <PanelNote>{t("catalog.noCollections")}</PanelNote>
      </PanelSection>
    </>
  );
}

/** Panneau droit : histogramme et fiche d'informations de la photo sélectionnée. */
export function PhotoInfoPanel({ scope }: { scope: string }) {
  const { t } = useTranslation();
  return (
    <>
      <PanelSection id={`${scope}.histogram`} title={t("info.histogram")}>
        <div className="histogram" aria-hidden="true" />
        <div className="histogram-exif" aria-label={t("info.exposure")}>
          <span>ISO —</span>
          <span>— mm</span>
          <span>f / —</span>
          <span>— s</span>
        </div>
      </PanelSection>
      <PanelSection id={`${scope}.metadata`} title={t("info.metadata")}>
        <FieldRow label={t("info.fileName")} />
        <FieldRow label={t("info.captureTime")} />
        <FieldRow label={t("info.camera")} />
        <FieldRow label={t("info.lens")} />
        <FieldRow label={t("info.exposure")} />
        <FieldRow label={t("info.iso")} />
      </PanelSection>
      <PanelSection id={`${scope}.keywords`} title={t("info.keywords")}>
        <PanelNote>{t("info.noSelection")}</PanelNote>
      </PanelSection>
    </>
  );
}
