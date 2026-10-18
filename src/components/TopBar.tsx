import { useTranslation } from "react-i18next";
import { useImport } from "../store/import";
import { showWelcome } from "../windows";
import { ModulePicker } from "./ModulePicker";

/** Barre supérieure : identité et bibliothèque à gauche, modules à droite. */
export function TopBar() {
  const { t } = useTranslation();
  const library = useImport((s) => s.libraries.find((l) => l.id === s.libraryId));
  return (
    <header className="top-bar">
      <div className="brand">
        <button
          type="button"
          className="brand-home"
          title={`${t("modules.home")} (Ctrl+Alt+0)`}
          onClick={() => void showWelcome()}
        >
          <span className="logo" aria-hidden="true" />
          <span className="brand-name">{t("app.name")}</span>
        </button>
        <span className="brand-library">
          {library
            ? library.isOnline
              ? library.name
              : t("app.libraryOffline", { name: library.name })
            : t("app.noLibrary")}
        </span>
      </div>
      <ModulePicker />
    </header>
  );
}
