import { Plus, Trash2 } from "lucide-react";
import { type KeyboardEvent, useState } from "react";
import { useTranslation } from "react-i18next";
import { ORIGINAL, freeName, useDevelop } from "../store/develop";
import { useLibrary } from "../store/library";
import { confirmWarning } from "../ui/confirm";
import { IconButton } from "./IconButton";
import { PanelSection } from "./PanelSection";

interface NameRowProps {
  label: string;
  selected?: boolean;
  /** Infobulle de la ligne. */
  hint?: string;
  onSelect: () => void;
  /** Double-clic : renommer ; renvoie `false` si le nom est refusé. */
  onRename?: (to: string) => Promise<boolean>;
  onDelete?: () => void;
  deleteLabel?: string;
}

/** Ligne de liste nommée : clic pour choisir, double-clic pour renommer. */
function NameRow({
  label,
  selected,
  hint,
  onSelect,
  onRename,
  onDelete,
  deleteLabel,
}: NameRowProps) {
  const [editing, setEditing] = useState<string | null>(null);

  const commit = async () => {
    if (editing === null || !onRename) return;
    const to = editing.trim();
    if (to === "" || to === label || (await onRename(to))) setEditing(null);
  };
  const onKey = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") void commit();
    if (e.key === "Escape") setEditing(null);
  };

  if (editing !== null) {
    return (
      <li className="develop-name-row">
        <input
          className="develop-name-input"
          value={editing}
          maxLength={64}
          autoFocus
          aria-label={label}
          onChange={(e) => setEditing(e.currentTarget.value)}
          onKeyDown={onKey}
          onBlur={() => void commit()}
        />
      </li>
    );
  }
  return (
    <li className="develop-name-row">
      <button
        type="button"
        className="list-row"
        aria-current={selected || undefined}
        title={hint}
        onClick={onSelect}
        onDoubleClick={onRename ? () => setEditing(label) : undefined}
      >
        <span className="list-row-label">{label}</span>
      </button>
      {onDelete && deleteLabel && (
        <span className="develop-name-delete">
          <IconButton icon={Trash2} label={deleteLabel} onClick={onDelete} />
        </span>
      )}
    </li>
  );
}

/** Versions : l'originale et les copies virtuelles de la photo. */
export function VersionsSection() {
  const { t } = useTranslation();
  const version = useDevelop((s) => s.version);
  const versions = useDevelop((s) => s.versions);
  const disabled = useDevelop((s) => s.settings === null);
  const { selectVersion } = useDevelop.getState();
  // La grille montre les copies : elle suit chaque création, renommage ou suppression.
  const reload = () => void useLibrary.getState().reloadMedia();
  const createVersion = async (name: string) => {
    await useDevelop.getState().createVersion(name);
    reload();
  };
  const renameVersion = async (from: string, to: string) => {
    const ok = await useDevelop.getState().renameVersion(from, to);
    reload();
    return ok;
  };
  const deleteVersion = async (name: string) => {
    await useDevelop.getState().deleteVersion(name);
    reload();
  };

  const create = () =>
    void createVersion(
      freeName(
        (n) => t("develop.versions.defaultName", { n }),
        versions.map((v) => v.name),
      ),
    );
  const remove = async (name: string) => {
    const ok = await confirmWarning(t("develop.versions.confirmDelete", { name }), {
      title: t("develop.versions.title"),
      okLabel: t("develop.versions.delete"),
      cancelLabel: t("trash.cancel"),
    });
    if (ok) await deleteVersion(name);
  };

  return (
    <PanelSection
      id="develop.versions"
      title={t("develop.versions.title")}
      aside={
        <span className="panel-section-aside develop-history-actions">
          <IconButton
            icon={Plus}
            label={t("develop.versions.create")}
            disabled={disabled}
            onClick={create}
          />
        </span>
      }
    >
      <ul className="develop-names">
        <NameRow
          label={t("develop.versions.original")}
          selected={version === ORIGINAL}
          onSelect={() => void selectVersion(ORIGINAL)}
        />
        {versions.map((v) => (
          <NameRow
            key={v.name}
            label={v.name}
            selected={version === v.name}
            hint={t("develop.renameHint")}
            onSelect={() => void selectVersion(v.name)}
            onRename={(to) => renameVersion(v.name, to)}
            onDelete={() => void remove(v.name)}
            deleteLabel={t("develop.versions.delete")}
          />
        ))}
      </ul>
    </PanelSection>
  );
}

/** Instantanés de la version affichée : un clic les applique (annulable). */
export function SnapshotsSection() {
  const { t } = useTranslation();
  const snapshots = useDevelop((s) => s.snapshots);
  const disabled = useDevelop((s) => s.settings === null);
  const { createSnapshot, applySnapshot, renameSnapshot, deleteSnapshot } = useDevelop.getState();

  const create = () =>
    void createSnapshot(
      freeName(
        (n) => t("develop.snapshots.defaultName", { n }),
        snapshots.map((s) => s.name),
      ),
    );
  const remove = async (name: string) => {
    const ok = await confirmWarning(t("develop.snapshots.confirmDelete", { name }), {
      title: t("develop.snapshots.title"),
      okLabel: t("develop.snapshots.delete"),
      cancelLabel: t("trash.cancel"),
    });
    if (ok) await deleteSnapshot(name);
  };

  return (
    <PanelSection
      id="develop.snapshots"
      title={t("develop.snapshots.title")}
      aside={
        <span className="panel-section-aside develop-history-actions">
          <IconButton
            icon={Plus}
            label={t("develop.snapshots.create")}
            disabled={disabled}
            onClick={create}
          />
        </span>
      }
    >
      {snapshots.length === 0 ? (
        <p className="panel-note">{t("develop.snapshots.empty")}</p>
      ) : (
        <ul className="develop-names">
          {snapshots.map((s) => (
            <NameRow
              key={s.name}
              label={s.name}
              hint={t("develop.snapshots.hint")}
              onSelect={() => void applySnapshot(s.name)}
              onRename={(to) => renameSnapshot(s.name, to)}
              onDelete={() => void remove(s.name)}
              deleteLabel={t("develop.snapshots.delete")}
            />
          ))}
        </ul>
      )}
    </PanelSection>
  );
}
