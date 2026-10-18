import { useTranslation } from "react-i18next";
import { drawsOwnMenus } from "../platform";
import { useImport } from "../store/import";
import { showWelcome } from "../windows";
import { MenuBar } from "./MenuBar";
import { ModulePicker } from "./ModulePicker";
import { WindowControls } from "./WindowControls";

/**
 * Barre supérieure, qui sert aussi de barre de titre : logo, menus (Windows),
 * bibliothèque au centre, modules, puis boutons de fenêtre (Windows).
 * Les zones `data-tauri-drag-region` déplacent la fenêtre ; un double clic
 * l'agrandit, comme une barre de titre native.
 */
export function TopBar() {
  const { t } = useTranslation();
  const library = useImport((s) => s.libraries.find((l) => l.id === s.libraryId));
  const libraryLabel = library
    ? library.isOnline
      ? library.name
      : t("app.libraryOffline", { name: library.name })
    : t("app.noLibrary");

  return (
    <header className="top-bar" data-tauri-drag-region>
      <button
        type="button"
        className="brand-home"
        title={`${t("modules.home")} (Ctrl+Alt+0)`}
        onClick={() => void showWelcome()}
      >
        <span className="logo" aria-hidden="true" />
        <span className="brand-name">{t("app.name")}</span>
      </button>
      {drawsOwnMenus && <MenuBar />}
      <div className="title-area" data-tauri-drag-region>
        <span className="brand-library" data-tauri-drag-region>
          {libraryLabel}
        </span>
      </div>
      <ModulePicker />
      {drawsOwnMenus && <WindowControls />}
    </header>
  );
}
