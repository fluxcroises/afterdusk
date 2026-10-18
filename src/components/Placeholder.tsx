import { useTranslation } from "react-i18next";

interface PlaceholderProps {
  titleKey: string;
  version: string;
}

/** Écran provisoire pour les étapes et vues pas encore livrées. */
export function Placeholder({ titleKey, version }: PlaceholderProps) {
  const { t } = useTranslation();
  return (
    <section className="placeholder">
      <h1>{t(titleKey)}</h1>
      <p>{t("placeholder.comingSoon", { version })}</p>
    </section>
  );
}
