import { useTranslation } from "react-i18next";
import { StepBar } from "./StepBar";

export function TopBar() {
  const { t } = useTranslation();
  return (
    <header className="top-bar">
      <div className="brand">
        <span className="logo" aria-hidden="true" />
        {t("app.name")}
      </div>
      <StepBar />
    </header>
  );
}
