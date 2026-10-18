import { useTranslation } from "react-i18next";

/** Barre d'état en bas de la fenêtre. */
export function StatusBar() {
  const { t } = useTranslation();
  return (
    <footer className="status-bar">
      <span>{t("status.selection", { count: 0, selected: 0 })}</span>
      <span className="status-bar-spacer" />
      <span title={t("status.safetyHint")}>{t("status.safety")}</span>
    </footer>
  );
}
