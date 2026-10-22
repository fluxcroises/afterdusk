import { invoke } from "@tauri-apps/api/core";
import { type SessionType, sessionLabel, useClients } from "../store/clients";
import { confirmWarning } from "../ui/confirm";
import { open } from "@tauri-apps/plugin-dialog";
import { useStudioLogo } from "../store/studioLogo";
import { useAnalysis } from "../store/analysis";
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

/** Ratés détectés après chaque import (préférence locale, oui par défaut). */
function AnalysisField() {
  const { t } = useTranslation();
  const auto = useAnalysis((s) => s.auto);
  return (
    <label className="check-row">
      <input
        type="checkbox"
        checked={auto}
        onChange={(e) => useAnalysis.getState().setAuto(e.currentTarget.checked)}
      />
      {t("settings.autoAnalysis")}
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
/** Règle d'import automatique (`import::auto::Rule`). */
interface AutoRule {
  id: number;
  bodySerial: string;
  camera: string;
  libraryName: string;
  lastUsedAt: string | null;
}

/** Boîtiers dont les cartes s'importent seules, avec les réglages de leur premier import. */
function AutoImportRules() {
  const { t, i18n } = useTranslation();
  const [rules, setRules] = useState<AutoRule[] | null>(null);
  const load = () =>
    invoke<AutoRule[]>("auto_import_rules")
      .then(setRules)
      .catch(() => setRules([]));
  useEffect(() => {
    void load();
  }, []);
  if (rules === null) return null;
  const remove = async (rule: AutoRule) => {
    await invoke("auto_import_rule_delete", { id: rule.id }).catch(() => undefined);
    await load();
  };
  const date = new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium" });
  return (
    <div className="auto-import-rules">
      <span className="field-row-label">{t("settings.autoImport")}</span>
      {rules.length === 0 ? (
        <p className="panel-note">{t("settings.autoImportNone")}</p>
      ) : (
        <ul>
          {rules.map((r) => (
            <li key={r.id}>
              <span>
                {t("settings.autoImportRule", { camera: r.camera, serial: r.bodySerial })}
                <span className="panel-note">
                  {" · "}
                  {[
                    r.libraryName,
                    r.lastUsedAt
                      ? t("settings.autoImportUsed", { date: date.format(new Date(r.lastUsedAt)) })
                      : null,
                  ]
                    .filter(Boolean)
                    .join(" · ")}
                </span>
              </span>
              <button
                type="button"
                className="button button-small"
                aria-label={t("settings.autoImportRemove", { camera: r.camera })}
                onClick={() => void remove(r)}
              >
                {t("sessions.delete")}
              </button>
            </li>
          ))}
        </ul>
      )}
      <p className="panel-note">{t("settings.autoImportNote")}</p>
    </div>
  );
}

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
      <AutoImportRules />
      <AnalysisField />
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

/** Logo du studio : en tête des documents PDF et comme filigrane. */
function StudioLogoField() {
  const { t } = useTranslation();
  const url = useStudioLogo((s) => s.url);
  const error = useStudioLogo((s) => s.error);
  useEffect(() => {
    void useStudioLogo.getState().load();
  }, []);
  const choose = async () => {
    const path = await open({
      multiple: false,
      title: t("settings.logoChoose"),
      filters: [{ name: t("settings.logoImages"), extensions: ["png", "jpg", "jpeg", "webp"] }],
    });
    if (typeof path === "string") await useStudioLogo.getState().set(path);
  };
  return (
    <div className="studio-logo">
      <span className="field-row-label">{t("settings.logo")}</span>
      <ErrorBanner error={error} onDismiss={() => useStudioLogo.setState({ error: null })} />
      <div className="studio-logo-preview">
        {url ? <img src={url} alt={t("settings.logo")} /> : <span>{t("settings.noLogo")}</span>}
      </div>
      <p className="panel-note">{t("settings.logoNote")}</p>
      <div className="settings-actions">
        <button type="button" className="button button-small" onClick={() => void choose()}>
          {t(url ? "settings.logoReplace" : "settings.logoChoose")}
        </button>
        {url && (
          <button
            type="button"
            className="button button-small"
            onClick={() => void useStudioLogo.getState().clear()}
          >
            {t("settings.logoRemove")}
          </button>
        )}
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
      <StudioLogoField />
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

/** Une ligne de type de séance : renommer (Entrée) ou retirer. */
function SessionTypeRow({ type }: { type: SessionType }) {
  const { t } = useTranslation();
  const label = sessionLabel(t, type);
  const [name, setName] = useState(label);
  const st = useClients.getState();
  const commit = () => {
    if (name.trim() && name.trim() !== label) void st.renameType(type.id, name);
    else setName(label);
  };
  const remove = async () => {
    const ok = await confirmWarning(t("sessions.confirmDelete", { name: label }), {
      title: t("settings.sessions"),
      okLabel: t("sessions.delete"),
      cancelLabel: t("trash.cancel"),
    });
    if (ok) await st.removeType(type.id);
  };
  return (
    <li className="session-type-row">
      <input
        type="text"
        aria-label={t("sessions.rename", { name: label })}
        value={name}
        maxLength={60}
        onChange={(e) => setName(e.currentTarget.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === "Enter") e.currentTarget.blur();
          if (e.key === "Escape") setName(label);
        }}
      />
      <button type="button" className="button button-small" onClick={() => void remove()}>
        {t("sessions.delete")}
      </button>
    </li>
  );
}

/** Réglages > Séances : la liste des types proposée pour les albums. */
export function SessionTypesSettings() {
  const { t } = useTranslation();
  const types = useClients((s) => s.sessionTypes);
  const [name, setName] = useState("");
  useEffect(() => {
    void useClients.getState().load();
  }, []);
  return (
    <div className="settings-form">
      <p className="panel-note">{t("sessions.note")}</p>
      <ul className="session-types">
        {types.map((s) => (
          // Une clé par nom affiché : la ligne repart du nom enregistré.
          <SessionTypeRow key={`${s.id}-${s.key ?? s.name}`} type={s} />
        ))}
      </ul>
      <form
        className="settings-actions"
        onSubmit={(e) => {
          e.preventDefault();
          if (!name.trim()) return;
          void useClients.getState().createType(name);
          setName("");
        }}
      >
        <input
          type="text"
          aria-label={t("sessions.newName")}
          placeholder={t("sessions.newName")}
          value={name}
          maxLength={60}
          onChange={(e) => setName(e.currentTarget.value)}
        />
        <button type="submit" className="button button-small" disabled={!name.trim()}>
          {t("sessions.add")}
        </button>
      </form>
    </div>
  );
}
