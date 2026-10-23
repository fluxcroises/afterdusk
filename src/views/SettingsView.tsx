import { ProfileChoice } from "../components/ProfileChoice";
import { useProfile } from "../store/profile";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { FieldRow, ListRow, PanelSection } from "../components/PanelSection";
import { ScreenLayout } from "../components/ScreenLayout";
import i18n, { type Language, resources } from "../i18n";
import { useLibrary } from "../store/library";
import { useMap } from "../store/map";
import {
  IdentityPage,
  ImportSettings,
  LibrarySettings,
  SessionTypesSettings,
} from "../settings/Pages";

/** Corbeille et cache : délai avant suppression définitive. */
function TrashSettings() {
  const { t } = useTranslation();
  const days = useLibrary((s) => s.retentionDays);
  const setRetentionDays = useLibrary((s) => s.setRetentionDays);
  const [draft, setDraft] = useState(String(days));
  const commit = () => {
    const value = Math.round(Number(draft));
    if (Number.isFinite(value) && value >= 1) void setRetentionDays(Math.min(value, 365));
    else setDraft(String(days));
  };
  return (
    <div className="settings-form">
      <label className="field-row">
        <span className="field-row-label">{t("settings.trashRetention")}</span>
        <span className="settings-inline">
          <input
            type="number"
            min={1}
            max={365}
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onBlur={commit}
            onKeyDown={(e) => e.key === "Enter" && commit()}
          />
          {t("settings.days", { count: Number(draft) || days })}
        </span>
      </label>
      <p className="panel-note">{t("settings.trashNote")}</p>
    </div>
  );
}

const CATEGORIES = ["general", "library", "import", "identity", "sessions", "trash"] as const;
type Category = (typeof CATEGORIES)[number];

export function SettingsView() {
  const { t } = useTranslation();
  const [category, setCategory] = useState<Category>("general");

  const left = (
    <PanelSection id="settings.categories" title={t("modules.settings")}>
      {CATEGORIES.map((c) => (
        <ListRow
          key={c}
          label={t(`settings.${c}`)}
          selected={c === category}
          onClick={() => setCategory(c)}
        />
      ))}
    </PanelSection>
  );

  const online = useMap((s) => s.online);
  const profile = useProfile((s) => s.profile);

  return (
    <ScreenLayout left={left}>
      <div className="settings-page">
        <h1 className="settings-title">{t(`settings.${category}`)}</h1>
        {category === "general" ? (
          <div className="settings-columns">
            <div className="settings-form">
              <label className="field-row">
                <span className="field-row-label">{t("settings.language")}</span>
                <select
                  value={i18n.language}
                  onChange={(e) => void i18n.changeLanguage(e.target.value as Language)}
                >
                  {Object.keys(resources).map((lng) => (
                    <option key={lng} value={lng}>
                      {t(`settings.languages.${lng}`)}
                    </option>
                  ))}
                </select>
              </label>
              <FieldRow label={t("settings.theme")} value={t("settings.themeDarkOnly")} />
              <label className="check-row">
                <input
                  type="checkbox"
                  checked={online}
                  onChange={(e) => useMap.getState().setOnline(e.currentTarget.checked)}
                />
                {t("settings.mapOnline")}
              </label>
              <p className="panel-note">{t("settings.mapOnlineNote")}</p>
            </div>
            <fieldset className="form-fieldset settings-form">
              <legend className="settings-block-title">{t("profiles.label")}</legend>
              <ProfileChoice value={profile} onChange={(p) => void useProfile.getState().set(p)} />
              <p className="panel-note">{t("profiles.note")}</p>
            </fieldset>
          </div>
        ) : category === "library" ? (
          <LibrarySettings />
        ) : category === "import" ? (
          <ImportSettings />
        ) : category === "identity" ? (
          <IdentityPage />
        ) : category === "sessions" ? (
          <SessionTypesSettings />
        ) : (
          <TrashSettings />
        )}
      </div>
    </ScreenLayout>
  );
}
