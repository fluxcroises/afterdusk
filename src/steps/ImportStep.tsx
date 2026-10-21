import { FolderOpen, HardDriveDownload, RefreshCw } from "lucide-react";
import { useTranslation } from "react-i18next";
import { newLibrary, pickFolder } from "../actions";
import { EmptyState } from "../components/EmptyState";
import { ErrorBanner } from "../components/ErrorBanner";
import { IconButton } from "../components/IconButton";
import { ImportGrid } from "../components/ImportGrid";
import { FieldRow, ListRow, PanelNote, PanelSection } from "../components/PanelSection";
import { ScreenLayout } from "../components/ScreenLayout";
import { deviceLabel, useDevices } from "../store/devices";
import { useLocate } from "../store/locate";
import { useImport } from "../store/import";

/**
 * Chemin affiché de droite à gauche (pour couper le début, pas la fin) : une
 * marque gauche-à-droite finale garde « X:\ » dans le bon ordre.
 */
function ltr(path: string): string {
  return path + "‎";
}

function SourcePanel() {
  const { t } = useTranslation();
  const source = useImport((s) => s.source);
  const scanning = useImport((s) => s.scanning);
  const running = useImport((s) => s.running);
  const scanSource = useImport((s) => s.scanSource);
  const resumable = useImport((s) => s.resumable);
  const resume = useImport((s) => s.resume);
  const devices = useDevices((s) => s.devices);

  const chooseFolder = async () => {
    const path = await pickFolder(t("import.chooseFolder"));
    if (path) await scanSource(path);
  };

  return (
    <>
      <PanelSection id="import.devices" title={t("import.devices")}>
        {devices.length === 0 ? (
          <PanelNote>{t("import.noDevice")}</PanelNote>
        ) : (
          devices.map((d) => (
            <ListRow
              key={d.root}
              label={deviceLabel(d)}
              hint={d.root}
              selected={source?.root === d.root}
              onClick={() => {
                if (scanning || running) return;
                useDevices.getState().handled(d.root);
                void scanSource(d.root);
              }}
            />
          ))
        )}
      </PanelSection>
      <PanelSection
        id="import.folders"
        title={t("import.folders")}
        aside={
          source && (
            <IconButton
              icon={RefreshCw}
              label={t("import.rescan")}
              disabled={scanning || running}
              onClick={() => void scanSource(source.root)}
            />
          )
        }
      >
        {source && (
          <p className="path-label" title={source.root}>
            {ltr(source.root)}
          </p>
        )}
        <button
          type="button"
          className="button button-secondary"
          disabled={scanning || running}
          onClick={() => void chooseFolder()}
        >
          <FolderOpen aria-hidden="true" strokeWidth={1.75} />
          {t("import.chooseFolder")}
        </button>
      </PanelSection>
      {resumable.length > 0 && (
        <PanelSection id="import.resumable" title={t("import.interrupted")}>
          {resumable.map((r) => (
            <div key={r.id} className="resumable-row">
              <span>
                {r.sourceLabel}
                <span className="list-row-count">
                  {t("import.remaining", { count: r.remaining, total: r.total })}
                </span>
              </span>
              <button
                type="button"
                className="button button-secondary button-small"
                disabled={running}
                onClick={() => void resume(r.id)}
              >
                {t("import.resume")}
              </button>
            </div>
          ))}
        </PanelSection>
      )}
    </>
  );
}

function DestinationPanel() {
  const { t } = useTranslation();
  const libraries = useImport((s) => s.libraries);
  const libraryId = useImport((s) => s.libraryId);
  const selectLibrary = useImport((s) => s.selectLibrary);
  const form = useImport((s) => s.form);
  const hasPairs = useImport(
    (s) => s.source?.days.some((d) => d.files.some((f) => f.companions.length > 0)) ?? false,
  );
  const update = useImport((s) => s.updateForm);
  const running = useImport((s) => s.running);
  const library = libraries.find((l) => l.id === libraryId);

  return (
    <fieldset className="form-fieldset" disabled={running}>
      <PanelSection id="import.library" title={t("import.library")}>
        {libraries.length > 0 ? (
          <label className="field-row">
            <span className="field-row-label">{t("import.libraryName")}</span>
            <select value={libraryId ?? ""} onChange={(e) => selectLibrary(Number(e.target.value))}>
              {libraries.map((l) => (
                <option key={l.id} value={l.id}>
                  {l.name}
                </option>
              ))}
            </select>
          </label>
        ) : (
          <PanelNote>{t("import.noLibrary")}</PanelNote>
        )}
        {library && (
          <p className="path-label" title={library.rootPath}>
            {ltr(library.rootPath)}
          </p>
        )}
        <button type="button" className="button button-secondary" onClick={() => void newLibrary()}>
          {t("import.newLibrary")}
        </button>
      </PanelSection>
      <PanelSection id="import.destination" title={t("import.destination")}>
        <label className="field-row">
          <span className="field-row-label">{t("import.album")}</span>
          <input
            type="text"
            value={form.albumName}
            placeholder={t("import.albumPlaceholder")}
            onChange={(e) => update({ albumName: e.target.value })}
          />
        </label>
        <label className="check-row">
          <input
            type="checkbox"
            checked={form.albumPerDay}
            onChange={(e) => update({ albumPerDay: e.target.checked })}
          />
          {t("import.albumPerDay")}
        </label>
        <FieldRow
          label={t("import.folderTemplate")}
          value={library?.folderTemplate ?? t("import.defaultFolderTemplate")}
        />
      </PanelSection>
      {hasPairs && (
        <PanelSection id="import.pairs" title={t("import.pairs")}>
          <label className="check-row">
            <input
              type="checkbox"
              checked={form.pairJpegs}
              onChange={(e) => update({ pairJpegs: e.target.checked })}
            />
            {t("import.pairJpegs")}
          </label>
          <PanelNote>{t("import.pairJpegsHint")}</PanelNote>
        </PanelSection>
      )}
      <PanelSection id="import.renaming" title={t("import.renaming")}>
        <label className="check-row">
          <input
            type="checkbox"
            checked={form.rename}
            onChange={(e) => update({ rename: e.target.checked })}
          />
          {t("import.renameFiles")}
        </label>
        <FieldRow
          label={t("import.renameTemplate")}
          value={
            form.rename
              ? (library?.renameTemplate ?? t("import.defaultRenameTemplate"))
              : t("import.keepOriginalName")
          }
        />
      </PanelSection>
      <PanelSection id="import.backup" title={t("import.backup")}>
        <label className="check-row">
          <input
            type="checkbox"
            checked={form.backup && !!library?.backupPath}
            disabled={!library?.backupPath}
            onChange={(e) => update({ backup: e.target.checked })}
          />
          {t("import.makeBackup")}
        </label>
        <FieldRow
          label={t("import.backupLocation")}
          value={library?.backupPath ?? t("import.noBackup")}
        />
      </PanelSection>
      <PanelSection id="import.metadata" title={t("import.metadata")}>
        <label className="field-row">
          <span className="field-row-label">{t("import.copyright")}</span>
          <input
            type="text"
            value={form.rights}
            onChange={(e) => update({ rights: e.target.value })}
          />
        </label>
        <label className="field-row">
          <span className="field-row-label">{t("import.keywords")}</span>
          <input
            type="text"
            value={form.keywords}
            placeholder={t("import.keywordsPlaceholder")}
            onChange={(e) => update({ keywords: e.target.value })}
          />
        </label>
      </PanelSection>
    </fieldset>
  );
}

function ImportFooter() {
  const { t } = useTranslation();
  const selected = useImport((s) => s.selected);
  const running = useImport((s) => s.running);
  const progress = useImport((s) => s.progress);
  const libraryId = useImport((s) => s.libraryId);
  const source = useImport((s) => s.source);
  const start = useImport((s) => s.start);
  const cancel = useImport((s) => s.cancel);
  const device = useDevices((s) => s.devices.find((d) => d.root === source?.root));
  const offline = useImport(
    (s) => s.libraries.find((l) => l.id === s.libraryId)?.isOnline === false,
  );
  // Libellé de la source dans l'historique : la carte, ou le nom du dossier.
  const sourceLabel = device
    ? deviceLabel(device)
    : source
      ? source.root.split(/[\\/]/).pop() || source.root
      : "";

  if (running) {
    const done = progress?.processed ?? 0;
    const total = progress?.total ?? selected.size;
    return (
      <div className="import-progress">
        <progress value={done} max={Math.max(total, 1)} />
        <span className="toolbar-status">{t("import.progress", { done, total })}</span>
        <button type="button" className="button" onClick={() => void cancel()}>
          {t("import.stop")}
        </button>
      </div>
    );
  }
  return (
    <button
      type="button"
      className="button button-primary"
      disabled={selected.size === 0 || libraryId === null || offline}
      title={offline ? t("offline.importDisabled") : undefined}
      onClick={() => void start(sourceLabel)}
    >
      {t("import.startCount", { count: selected.size })}
    </button>
  );
}

function StatusLine() {
  const { t } = useTranslation();
  const running = useImport((s) => s.running);
  const progress = useImport((s) => s.progress);
  const lastResult = useImport((s) => s.lastResult);
  const unlocated = useImport((s) => s.unlocated);
  const selected = useImport((s) => s.selected);
  const selectAll = useImport((s) => s.selectAll);
  const source = useImport((s) => s.source);
  const total = source?.days.reduce((n, d) => n + d.files.length, 0) ?? 0;

  let status = t("import.selectedOf", { count: selected.size, total });
  if (running && progress?.currentFile) {
    status = t("import.copying", { file: progress.currentFile });
  } else if (lastResult?.outcome && progress) {
    status = t(lastResult.outcome === "done" ? "import.summary" : "import.summaryInterrupted", {
      imported: progress.imported,
      duplicates: progress.duplicates,
      failed: progress.failed,
    });
  }

  return (
    <>
      <button
        type="button"
        className="button button-small"
        disabled={!source || running}
        onClick={() => selectAll(true)}
      >
        {t("import.selectAll")}
      </button>
      <button
        type="button"
        className="button button-small"
        disabled={!source || running}
        onClick={() => selectAll(false)}
      >
        {t("import.selectNone")}
      </button>
      <span className="toolbar-spacer" />
      <span className="toolbar-status">{status}</span>
      {!running && unlocated.length > 0 && (
        <button
          type="button"
          className="button button-small"
          onClick={() => void useLocate.getState().open(unlocated)}
        >
          {t("import.locate", { count: unlocated.length })}
        </button>
      )}
    </>
  );
}

export function ImportStep() {
  const { t } = useTranslation();
  const source = useImport((s) => s.source);
  const scanning = useImport((s) => s.scanning);
  const error = useImport((s) => s.error);
  const dismissError = useImport((s) => s.dismissError);

  let content;
  if (scanning) {
    content = <EmptyState icon={RefreshCw} title={t("import.scanning")} />;
  } else if (source && source.days.length > 0) {
    content = <ImportGrid />;
  } else if (source) {
    content = <EmptyState icon={HardDriveDownload} title={t("import.sourceEmpty")} />;
  } else {
    content = <EmptyState icon={HardDriveDownload} title={t("import.emptyTitle")} />;
  }

  return (
    <ScreenLayout
      left={<SourcePanel />}
      right={<DestinationPanel />}
      rightFooter={<ImportFooter />}
      toolbar={<StatusLine />}
    >
      <ErrorBanner error={error} onDismiss={dismissError} />
      {content}
    </ScreenLayout>
  );
}
