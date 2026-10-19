import { TriangleAlert } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { CommandError } from "../api/core";

interface ErrorBannerProps {
  error: CommandError | null;
  onDismiss: () => void;
}

/** Erreur du cœur, traduite par son code ; le détail technique en infobulle. */
export function ErrorBanner({ error, onDismiss }: ErrorBannerProps) {
  const { t } = useTranslation();
  if (!error) return null;
  return (
    <div className="banner banner-error" role="alert">
      <TriangleAlert aria-hidden="true" strokeWidth={1.75} />
      <span title={error.detail}>
        {t(`errors.${error.code}`, { defaultValue: t("errors.internal") })}
      </span>
      <button type="button" className="button button-small" onClick={onDismiss}>
        {t("import.dismiss")}
      </button>
    </div>
  );
}
