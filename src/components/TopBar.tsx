import { useTranslation } from "react-i18next";
import { useImport } from "../store/import";
import { ModulePicker } from "./ModulePicker";

/** Barre supérieure : identité et bibliothèque à gauche, modules à droite. */
export function TopBar() {
  const { t } = useTranslation();
  const library = useImport((s) => s.libraries.find((l) => l.id === s.libraryId));
  return (
    <header className="top-bar">
      <div className="brand">
        <span className="logo" aria-hidden="true" />
        <span className="brand-name">{t("app.name")}</span>
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
