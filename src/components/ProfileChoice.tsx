import { useTranslation } from "react-i18next";
import { PROFILES, type Profile } from "../ui/profiles";

/**
 * Choix du profil d'interface en trois cartes : nom, ce qu'il montre. Sert
 * à l'assistant de bienvenue et à Réglages > Général.
 */
export function ProfileChoice({
  value,
  onChange,
}: {
  value: Profile;
  onChange: (profile: Profile) => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="profile-choice" role="radiogroup" aria-label={t("profiles.label")}>
      {PROFILES.map((p) => (
        <label key={p} className="profile-card" data-checked={value === p || undefined}>
          <input
            type="radio"
            name="interface-profile"
            value={p}
            checked={value === p}
            onChange={() => onChange(p)}
          />
          <span className="profile-name">{t(`profiles.names.${p}`)}</span>
          <span className="profile-for">{t(`profiles.for.${p}`)}</span>
          <span className="profile-shows">{t(`profiles.shows.${p}`)}</span>
        </label>
      ))}
    </div>
  );
}
