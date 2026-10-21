import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { dueCount, useReminders } from "../store/maintenance";
import {
  AVAILABLE_IN,
  STEPS,
  type Screen,
  VIEWS,
  isLater,
  useNavigation,
} from "../store/navigation";

/** Sélecteur de modules à la Lightroom : les étapes du flux, puis les vues transverses. */
export function ModulePicker() {
  const { t } = useTranslation();
  const screen = useNavigation((s) => s.screen);
  const goTo = useNavigation((s) => s.goTo);
  const due = useReminders((s) => dueCount(s.reminders));

  useEffect(() => {
    void useReminders.getState().load();
  }, []);

  const item = (target: Screen) => {
    const later = isLater(target);
    const availability = later
      ? t("modules.availableIn", { version: AVAILABLE_IN[target] })
      : undefined;
    const badge = target === "gear" && due > 0;
    const label = availability
      ? `${t(`modules.${target}`)}, ${availability}`
      : badge
        ? `${t(`modules.${target}`)}, ${t("maintenance.dueCount", { count: due })}`
        : undefined;
    return (
      <li key={target}>
        <button
          type="button"
          className="module"
          data-later={later || undefined}
          aria-current={screen === target ? "page" : undefined}
          // La version prévue fait partie du nom lu au clavier, pas seulement de l'infobulle.
          aria-label={label}
          title={availability}
          onClick={() => goTo(target)}
        >
          {t(`modules.${target}`)}
          {badge && (
            <span className="module-badge" aria-hidden="true">
              {due}
            </span>
          )}
        </button>
      </li>
    );
  };

  return (
    <nav className="module-picker" aria-label={t("modules.label")}>
      <ol className="module-group">{STEPS.map(item)}</ol>
      <span className="module-separator" aria-hidden="true" />
      <ul className="module-group">{VIEWS.map(item)}</ul>
    </nav>
  );
}
