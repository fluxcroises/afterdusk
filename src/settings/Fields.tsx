import { Check, Cloud, FolderOpen, TriangleAlert } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { pickFolder } from "../actions";
import type { Identity } from "../api/core";
import {
  FOLDER_PRESETS,
  RENAME_PRESETS,
  cloudBackupPath,
  cloudOf,
  defaultCopyright,
  useCloudFolders,
  useSameVolume,
  useTemplatePreview,
} from "./presets";

/** Dossier choisi par le sélecteur natif, avec « Choisir… » et éventuellement « Retirer ». */
export function FolderField({
  label,
  path,
  placeholder,
  onChange,
  onClear,
}: {
  label: string;
  path: string | null;
  placeholder?: string;
  onChange: (path: string) => void;
  onClear?: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="folder-field">
      <span className="field-row-label">{label}</span>
      <span className="folder-field-path" title={path ?? undefined}>
        {path ?? placeholder ?? t("settings.none")}
      </span>
      <button
        type="button"
        className="button button-small folder-field-button"
        onClick={async () => {
          const chosen = await pickFolder(label);
          if (chosen) onChange(chosen);
        }}
      >
        <FolderOpen aria-hidden="true" strokeWidth={1.75} />
        {t("settings.choose")}
      </button>
      {onClear && path && (
        <button type="button" className="button button-small" onClick={onClear}>
          {t("settings.remove")}
        </button>
      )}
    </div>
  );
}

/** Dossiers cloud trouvés sur l'ordinateur, en un clic. */
export function CloudSuggestions({
  current,
  onPick,
}: {
  current: string | null;
  onPick: (path: string) => void;
}) {
  const { t } = useTranslation();
  const folders = useCloudFolders();
  if (folders.length === 0) return null;
  return (
    <div className="cloud-suggestions" role="group" aria-label={t("settings.cloudFolders")}>
      <span className="field-row-label">{t("settings.cloudFolders")}</span>
      {folders.map((f) => {
        const path = cloudBackupPath(f);
        return (
          <button
            key={f.path}
            type="button"
            className="button button-small cloud-button"
            aria-pressed={current === path}
            title={path}
            onClick={() => onPick(path)}
          >
            {current === path ? (
              <Check aria-hidden="true" strokeWidth={2} />
            ) : (
              <Cloud aria-hidden="true" strokeWidth={1.75} />
            )}
            {f.label}
          </button>
        );
      })}
    </div>
  );
}

/**
 * Une sauvegarde sur le disque de la bibliothèque ne protège pas d'une panne,
 * sauf si un service de cloud l'envoie en ligne : on le dit alors.
 */
export function BackupWarning({ root, backup }: { root: string | null; backup: string | null }) {
  const { t } = useTranslation();
  const same = useSameVolume(root, backup);
  const cloud = cloudOf(useCloudFolders(), backup);
  if (cloud) {
    return (
      <p className="settings-info" role="note">
        <Cloud aria-hidden="true" strokeWidth={1.75} />
        {t("settings.cloudBackup", { service: cloud.label })}
      </p>
    );
  }
  if (!same) return null;
  return (
    <p className="settings-warning" role="note">
      <TriangleAlert aria-hidden="true" strokeWidth={1.75} />
      {t("settings.sameDisk")}
    </p>
  );
}

/** Choix du rangement et du renommage, avec un exemple de résultat. */
export function TemplateChoice({
  folderTemplate,
  renameTemplate,
  onChange,
}: {
  folderTemplate: string;
  renameTemplate: string | null;
  onChange: (folderTemplate: string, renameTemplate: string | null) => void;
}) {
  const { t } = useTranslation();
  // Le mode personnalisé est un choix à part : un modèle saisi peut coïncider
  // avec un modèle proposé sans que l'on quitte la saisie.
  const [customMode, setCustomMode] = useState(
    () => !FOLDER_PRESETS.some((p) => p.template === folderTemplate),
  );
  const custom = customMode;
  const { path, error } = useTemplatePreview(
    folderTemplate,
    renameTemplate,
    t("settings.exampleAlbum"),
  );
  return (
    <div className="settings-form">
      {/* Dossiers et noms côte à côte : l'assistant tient sans défiler. */}
      <div className="template-columns">
        <fieldset className="choice-list">
          <legend className="settings-block-title">{t("settings.folders")}</legend>
          {FOLDER_PRESETS.map((p) => (
            <label key={p.id} className="check-row">
              <input
                type="radio"
                name="folder-template"
                checked={!custom && folderTemplate === p.template}
                onChange={() => {
                  setCustomMode(false);
                  onChange(p.template, renameTemplate);
                }}
              />
              {t(`settings.folderPresets.${p.id}`)}
            </label>
          ))}
          {/* Le modèle se saisit sur la même ligne : l'assistant tient sans défiler. */}
          <div className="custom-template-row">
            <label className="check-row">
              <input
                type="radio"
                name="folder-template"
                checked={custom}
                // On part du modèle en cours, à modifier.
                onChange={() => setCustomMode(true)}
              />
              {t("settings.folderPresets.custom")}
            </label>
            {custom && (
              <input
                type="text"
                className="template-input"
                aria-label={t("settings.folderTemplate")}
                // Les jetons en infobulle : la liste en clair ferait défiler l'assistant.
                title={t("settings.tokensHint")}
                value={folderTemplate}
                onChange={(e) => onChange(e.target.value, renameTemplate)}
              />
            )}
          </div>
        </fieldset>
        <fieldset className="choice-list">
          <legend className="settings-block-title">{t("settings.fileNames")}</legend>
          {RENAME_PRESETS.map((p) => (
            <label key={p.id} className="check-row">
              <input
                type="radio"
                name="rename-template"
                checked={
                  p.template === null
                    ? renameTemplate === null
                    : renameTemplate !== null && renameTemplate === p.template
                }
                onChange={() => onChange(folderTemplate, p.template)}
              />
              {t(`settings.renamePresets.${p.id}`)}
            </label>
          ))}
        </fieldset>
      </div>
      <p className="template-preview">
        <span className="field-row-label">{t("settings.example")}</span>
        {error ? (
          <span className="settings-error">{t(`errors.${error}`)}</span>
        ) : (
          path !== null && <code>{path}</code>
        )}
      </p>
    </div>
  );
}

/** Nom, copyright (proposé à partir du nom) et site du photographe. */
export function IdentityFields({
  identity,
  onChange,
}: {
  identity: Identity;
  onChange: (identity: Identity) => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="settings-form">
      <label className="field-row">
        <span className="field-row-label">{t("settings.identityName")}</span>
        <input
          type="text"
          value={identity.name}
          autoComplete="name"
          onChange={(e) => {
            const name = e.target.value;
            // Le copyright suit le nom tant qu'il n'a pas été personnalisé.
            const follows =
              !identity.copyright || identity.copyright === defaultCopyright(identity.name);
            onChange({
              ...identity,
              name,
              copyright: follows ? defaultCopyright(name) : identity.copyright,
            });
          }}
        />
      </label>
      <label className="field-row">
        <span className="field-row-label">{t("settings.identityCopyright")}</span>
        <input
          type="text"
          value={identity.copyright}
          onChange={(e) => onChange({ ...identity, copyright: e.target.value })}
        />
      </label>
      <label className="field-row">
        <span className="field-row-label">{t("settings.identityWebsite")}</span>
        <input
          type="text"
          inputMode="url"
          value={identity.website}
          placeholder="https://"
          onChange={(e) => onChange({ ...identity, website: e.target.value })}
        />
      </label>
      <label className="field-row">
        <span className="field-row-label">{t("settings.identityEmail")}</span>
        <input
          type="email"
          autoComplete="email"
          value={identity.email ?? ""}
          onChange={(e) => onChange({ ...identity, email: e.target.value })}
        />
      </label>
    </div>
  );
}
