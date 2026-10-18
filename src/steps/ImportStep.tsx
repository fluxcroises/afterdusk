import { HardDriveDownload } from "lucide-react";
import { useTranslation } from "react-i18next";
import { EmptyState } from "../components/EmptyState";
import { FieldRow, PanelNote, PanelSection } from "../components/PanelSection";
import { ScreenLayout } from "../components/ScreenLayout";

export function ImportStep() {
  const { t } = useTranslation();

  const left = (
    <>
      <PanelSection id="import.devices" title={t("import.devices")}>
        <PanelNote>{t("import.noDevice")}</PanelNote>
      </PanelSection>
      <PanelSection id="import.folders" title={t("import.folders")}>
        <PanelNote>{t("import.noFolder")}</PanelNote>
      </PanelSection>
    </>
  );

  const right = (
    <>
      <PanelSection id="import.destination" title={t("import.destination")}>
        <FieldRow label={t("import.album")} />
        <FieldRow label={t("import.folderTemplate")} value={t("import.defaultFolderTemplate")} />
      </PanelSection>
      <PanelSection id="import.renaming" title={t("import.renaming")}>
        <FieldRow label={t("import.renameTemplate")} value={t("import.keepOriginalName")} />
      </PanelSection>
      <PanelSection id="import.backup" title={t("import.backup")}>
        <FieldRow label={t("import.backupLocation")} value={t("import.noBackup")} />
      </PanelSection>
      <PanelSection id="import.metadata" title={t("import.metadata")}>
        <FieldRow label={t("import.copyright")} />
        <FieldRow label={t("import.keywords")} />
      </PanelSection>
    </>
  );

  const footer = (
    <button type="button" className="button button-primary" disabled>
      {t("import.start")}
    </button>
  );

  return (
    <ScreenLayout
      left={left}
      right={right}
      rightFooter={footer}
      toolbar={<span className="toolbar-status">{t("import.selectedCount", { count: 0 })}</span>}
    >
      <EmptyState icon={HardDriveDownload} title={t("import.emptyTitle")}>
        {t("import.emptyText")}
      </EmptyState>
    </ScreenLayout>
  );
}
