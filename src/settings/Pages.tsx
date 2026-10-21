import { useBursts } from "../store/bursts";
import { isTauri } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { type CommandError, type Identity, api, toCommandError } from "../api/core";
import { ErrorBanner } from "../components/ErrorBanner";
import { FieldRow } from "../components/PanelSection";
import { type InsertBehavior, useDevices } from "../store/devices";
import { useImport } from "../store/import";
import {
  BackupWarning,
  CloudSuggestions,
  FolderField,
  IdentityFields,
  TemplateChoice,
} from "./Fields";

function useCurrentLibrary() {
  const libraries = useImport((s) => s.libraries);
  const libraryId = useImport((s) => s.libraryId);
  return libraries.find((l) => l.id === libraryId) ?? null;
}

/** Enregistre un réglage, recharge les bibliothèques et signale l'erreur éventuelle. */
function useSave() {
  const [error, setError] = useState<CommandError | null>(null);
  const [saved, setSaved] = useState(false);
  const save = async (run: () => Promise<unknown>) => {
    setError(null);
    setSaved(false);
    try {
      await run();
      await useImport.getState().loadLibraries();
      setSaved(true);
    } catch (e) {
      setError(toCommandError(e));
    }
  };
  return { error, saved, save, dismiss: () => setError(null) };
}

/** Réglages > Bibliothèques : emplacement et sauvegarde. */
export function LibrarySettings() {
  const { t } = useTranslation();
  const library = useCurrentLibrary();
  const { error, save, dismiss } = useSave();
  if (!library) return <p className="panel-note">{t("app.noLibrary")}</p>;
  return (
    <div className="settings-form">
      <ErrorBanner error={error} onDismiss={dismiss} />
      <FieldRow label={t("settings.libraryName")} value={library.name} />
      <FieldRow label={t("settings.libraryLocation")} value={library.rootPath} />
      <FolderField
        label={t("settings.backupFolder")}
        path={library.backupPath}
        placeholder={t("onboarding.backup.none")}
        onChange={(backup) => void save(() => api.librarySetBackup(library.id, backup))}
        onClear={() => void save(() => api.librarySetBackup(library.id, null))}
      />
      <CloudSuggestions
        current={library.backupPath}
        onPick={(backup) => void save(() => api.librarySetBackup(library.id, backup))}
      />
      <BackupWarning root={library.rootPath} backup={library.backupPath} />
      <p className="panel-note">{t("settings.backupHint")}</p>
    </div>
  );
}

/** Ce que fait Afterdusk quand on insère une carte (§4.10). */
function InsertBehaviorField() {
  const { t } = useTranslation();
  const behavior = useDevices((s) => s.behavior);
  return (
    <label className="field-row">
      <span className="field-row-label">{t("settings.onInsert")}</span>
      <select
        value={behavior}
        onChange={(e) => void useDevices.getState().setBehavior(e.target.value as InsertBehavior)}
      >
        {(["propose", "open", "nothing"] as const).map((b) => (
          <option key={b} value={b}>
            {t(`settings.insert.${b}`)}
          </option>
        ))}
      </select>
    </label>
  );
}

/** Rafales regroupées après chaque import (préférence locale, oui par défaut). */
function BurstsField() {
  const { t } = useTranslation();
  const auto = useBursts((s) => s.auto);
  return (
    <label className="check-row">
      <input
        type="checkbox"
        checked={auto}
        onChange={(e) => useBursts.getState().setAuto(e.currentTarget.checked)}
      />
      {t("settings.autoBursts")}
    </label>
  );
}

/** Réglages > Import : modèles de rangement et de renommage de la bibliothèque. */
export function ImportSettings() {
  const { t } = useTranslation();
  const library = useCurrentLibrary();
  const [draft, setDraft] = useState<{ folder: string; rename: string | null } | null>(null);
  const { error, saved, save, dismiss } = useSave();
  if (!library) return <p className="panel-note">{t("app.noLibrary")}</p>;
  const folder = draft?.folder ?? library.folderTemplate;
  const rename = draft ? draft.rename : library.renameTemplate;
  const changed = folder !== library.folderTemplate || rename !== library.renameTemplate;
  return (
    <div className="settings-form">
      <ErrorBanner error={error} onDismiss={dismiss} />
      <TemplateChoice
        folderTemplate={folder}
        renameTemplate={rename}
        onChange={(f, r) => setDraft({ folder: f, rename: r })}
      />
      <p className="panel-note">{t("settings.templatesHint")}</p>
      <InsertBehaviorField />
      <BurstsField />
      <div className="settings-actions">
        <button
          type="button"
          className="button button-primary button-inline"
          disabled={!changed}
          onClick={() =>
            void save(async () => {
              await api.librarySetTemplates(library.id, folder, rename);
              setDraft(null);
            })
          }
        >
          {t("settings.save")}
        </button>
        {saved && !changed && <span className="settings-saved">{t("settings.saved")}</span>}
      </div>
    </div>
  );
}

/** Réglages > Identité : nom, copyright et site du photographe. */
export function IdentitySettings({ initial }: { initial: Identity }) {
  const { t } = useTranslation();
  const [identity, setIdentity] = useState(initial);
  const [stored, setStored] = useState(initial);
  const { error, saved, save, dismiss } = useSave();
  const changed = JSON.stringify(identity) !== JSON.stringify(stored);
  return (
    <div className="settings-form">
      <ErrorBanner error={error} onDismiss={dismiss} />
      <IdentityFields identity={identity} onChange={setIdentity} />
      <div className="settings-actions">
        <button
          type="button"
          className="button button-primary button-inline"
          disabled={!changed}
          onClick={() =>
            void save(async () => {
              const result = await api.identitySet(identity);
              setIdentity(result);
              setStored(result);
            })
          }
        >
          {t("settings.save")}
        </button>
        {saved && !changed && <span className="settings-saved">{t("settings.saved")}</span>}
      </div>
    </div>
  );
}

/** Identité lue dans la base, puis modifiable. */
export function IdentityPage() {
  // Hors de l'application (navigateur de développement) : identité vide.
  const [identity, setIdentity] = useState<Identity | null>(() =>
    isTauri() ? null : { name: "", copyright: "", website: "" },
  );
  useEffect(() => {
    if (!isTauri()) return;
    let current = true;
    api
      .identityGet()
      .then((i) => current && setIdentity(i))
      .catch(() => current && setIdentity({ name: "", copyright: "", website: "" }));
    return () => {
      current = false;
    };
  }, []);
  return identity ? <IdentitySettings initial={identity} /> : null;
}
