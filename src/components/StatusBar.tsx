import { useTranslation } from "react-i18next";
import { useLibrary } from "../store/library";

/** Barre d'état en bas de la fenêtre. */
export function StatusBar() {
  const { t } = useTranslation();
  const count = useLibrary((s) => s.media.length);
  const selected = useLibrary((s) => (s.selectedId === null ? 0 : 1));
  return (
    <footer className="status-bar">
      <span>{t("status.selection", { count, selected })}</span>
      <span className="status-bar-spacer" />
      <span title={t("status.safetyHint")}>{t("status.safety")}</span>
    </footer>
  );
}
