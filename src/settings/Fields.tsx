import { FolderOpen, TriangleAlert } from "lucide-react";
import { useTranslation } from "react-i18next";
import { pickFolder } from "../actions";
import type { Identity } from "../api/core";
import {
  FOLDER_PRESETS,
  RENAME_PRESETS,
  defaultCopyright,
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

/** Une sauvegarde sur le disque de la bibliothèque ne protège pas d'une panne. */
export function BackupWarning({ root, backup }: { root: string | null; backup: string | null }) {
  const { t } = useTranslation();
  const same = useSameVolume(root, backup);
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
  const custom = !FOLDER_PRESETS.some((p) => p.template === folderTemplate);
  const { path, error } = useTemplatePreview(
    folderTemplate,
    renameTemplate,
    t("settings.exampleAlbum"),
  );
  return (
    <div className="settings-form">
      <fieldset className="choice-list">
        <legend className="field-row-label">{t("settings.folders")}</legend>
        {FOLDER_PRESETS.map((p) => (
          <label key={p.id} className="check-row">
            <input
              type="radio"
              name="folder-template"
              checked={folderTemplate === p.template}
              onChange={() => onChange(p.template, renameTemplate)}
            />
            {t(`settings.folderPresets.${p.id}`)}
          </label>
        ))}
        <label className="check-row">
          <input
            type="radio"
            name="folder-template"
            checked={custom}
            onChange={() => onChange(custom ? folderTemplate : "{yyyy}/{album}", renameTemplate)}
          />
          {t("settings.folderPresets.custom")}
        </label>
        {custom && (
          <input
            type="text"
            className="template-input"
            aria-label={t("settings.folderTemplate")}
            value={folderTemplate}
            onChange={(e) => onChange(e.target.value, renameTemplate)}
          />
        )}
      </fieldset>
      <fieldset className="choice-list">
        <legend className="field-row-label">{t("settings.fileNames")}</legend>
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
      <p className="template-preview">
        <span className="field-row-label">{t("settings.example")}</span>
        {error ? (
          <span className="settings-error">{t(`errors.${error}`)}</span>
        ) : (
          <code>{path ?? "—"}</code>
        )}
      </p>
      <p className="panel-note">{t("settings.tokensHint")}</p>
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
      <p className="panel-note">{t("settings.identityHint")}</p>
    </div>
  );
}
