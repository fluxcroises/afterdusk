import { ClipboardPaste, Copy } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { isMac } from "../platform";
import { useDevelop } from "../store/develop";
import { useLibrary } from "../store/library";
import { pasteTargets } from "../ui/paste";
import { COPY_DEFAULT_KEYS, COPY_SECTIONS, pathsOf } from "../ui/sections";
import { IconButton } from "./IconButton";
import { SectionPicker } from "./SettingsSections";

/** Copier (Ctrl+Maj+C) et coller (Ctrl+Maj+V) les réglages, dans la barre du Retoucher. */
export function CopyPasteControls() {
  const { t } = useTranslation();
  const disabled = useDevelop((s) => s.settings === null);
  const copied = useDevelop((s) => s.clipboard !== null);
  const count = useLibrary((s) => Math.max(1, s.selectedIds.length));
  const mod = isMac ? "⌘⇧" : t("develop.copy.modifiers");
  return (
    <div className="toolbar-group">
      <IconButton
        icon={Copy}
        label={t("develop.copy.open")}
        shortcut={`${mod}C`}
        disabled={disabled}
        onClick={() => useDevelop.getState().setCopyOpen(true)}
      />
      <IconButton
        icon={ClipboardPaste}
        label={t("develop.paste.label", { count })}
        shortcut={`${mod}V`}
        disabled={disabled || !copied}
        onClick={() => void useDevelop.getState().pasteSettings(pasteTargets())}
      />
    </div>
  );
}

/** Familles cochées à l'ouverture : le dernier choix, sinon le défaut. */
function initialKeys(previous: { controls: string[] } | null): string[] {
  if (!previous) return COPY_DEFAULT_KEYS;
  return COPY_SECTIONS.filter((o) => o.paths.every((p) => previous.controls.includes(p))).map(
    (o) => o.key,
  );
}

/** Boîte « Copier les réglages » : familles à transmettre. */
export function CopyDialog() {
  const open = useDevelop((s) => s.copyOpen);
  return open ? <CopyForm /> : null;
}

function CopyForm() {
  const { t } = useTranslation();
  const [keys, setKeys] = useState<string[]>(() => initialKeys(useDevelop.getState().clipboard));

  useEffect(() => {
    const escape = (e: KeyboardEvent) => {
      if (e.key === "Escape") useDevelop.getState().setCopyOpen(false);
    };
    window.addEventListener("keydown", escape);
    return () => window.removeEventListener("keydown", escape);
  }, []);

  const close = () => useDevelop.getState().setCopyOpen(false);
  return (
    <div className="modal-backdrop" onMouseDown={(e) => e.target === e.currentTarget && close()}>
      <form
        className="copy-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="copy-title"
        onSubmit={(e) => {
          e.preventDefault();
          useDevelop.getState().copySettings(pathsOf(COPY_SECTIONS, keys));
        }}
      >
        <h2 id="copy-title" className="copy-title">
          {t("develop.copy.title")}
        </h2>
        <p className="panel-note">{t("develop.copy.hint")}</p>
        <SectionPicker options={COPY_SECTIONS} checked={keys} onChange={setKeys} />
        <div className="develop-crop-actions">
          <span className="toolbar-spacer" />
          <button type="button" className="button button-small" onClick={close}>
            {t("develop.crop.cancel")}
          </button>
          <button
            type="submit"
            className="button button-small button-primary"
            disabled={keys.length === 0}
            autoFocus
          >
            {t("develop.copy.confirm")}
          </button>
        </div>
      </form>
    </div>
  );
}
