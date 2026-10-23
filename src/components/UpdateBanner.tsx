import { Download } from "lucide-react";
import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { useUpdates } from "../store/updates";
import { ErrorBanner } from "./ErrorBanner";

/**
 * Nouvelle version trouvée au démarrage (décision 0013) : proposée ici,
 * installée seulement sur clic, puis Afterdusk redémarre.
 */
export function UpdateBanner() {
  const { t } = useTranslation();
  const available = useUpdates((s) => s.available);
  const dismissed = useUpdates((s) => s.dismissed);
  const progress = useUpdates((s) => s.progress);
  const error = useUpdates((s) => s.error);

  useEffect(() => {
    void useUpdates.getState().check();
  }, []);

  if (error) {
    return <ErrorBanner error={error} onDismiss={() => useUpdates.setState({ error: null })} />;
  }
  if (!available || dismissed) return null;
  if (progress) {
    const percent = progress.total
      ? Math.min(100, Math.round((progress.downloaded / progress.total) * 100))
      : null;
    return (
      <div className="banner banner-info" role="status">
        <Download aria-hidden="true" strokeWidth={1.75} />
        <span>
          {percent === null
            ? t("updates.downloading")
            : t("updates.downloadingPercent", { percent })}
        </span>
      </div>
    );
  }
  return (
    <div className="banner banner-info" role="status">
      <Download aria-hidden="true" strokeWidth={1.75} />
      <span>{t("updates.available", { version: available.version })}</span>
      <button
        type="button"
        className="button button-small button-primary"
        onClick={() => void useUpdates.getState().install()}
      >
        {t("updates.install")}
      </button>
      <button
        type="button"
        className="button button-small"
        onClick={() => useUpdates.getState().dismiss()}
      >
        {t("updates.later")}
      </button>
    </div>
  );
}
