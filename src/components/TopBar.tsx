import { useTranslation } from "react-i18next";
import { ModulePicker } from "./ModulePicker";

/** Barre supérieure : identité et bibliothèque à gauche, modules à droite. */
export function TopBar() {
  const { t } = useTranslation();
  return (
    <header className="top-bar">
      <div className="brand">
        <span className="logo" aria-hidden="true" />
        <span className="brand-name">{t("app.name")}</span>
        <span className="brand-library">{t("app.noLibrary")}</span>
      </div>
      <ModulePicker />
    </header>
  );
}
