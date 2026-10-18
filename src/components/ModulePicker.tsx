import { useTranslation } from "react-i18next";
import {
  AVAILABLE_IN,
  CURRENT_VERSION,
  STEPS,
  type Screen,
  VIEWS,
  useNavigation,
} from "../store/navigation";

/** Sélecteur de modules à la Lightroom : les étapes du flux, puis les vues transverses. */
export function ModulePicker() {
  const { t } = useTranslation();
  const screen = useNavigation((s) => s.screen);
  const goTo = useNavigation((s) => s.goTo);

  const item = (target: Screen) => {
    const later = AVAILABLE_IN[target] !== CURRENT_VERSION;
    return (
      <li key={target}>
        <button
          type="button"
          className="module"
          data-later={later || undefined}
          aria-current={screen === target ? "page" : undefined}
          title={later ? t("modules.availableIn", { version: AVAILABLE_IN[target] }) : undefined}
          onClick={() => goTo(target)}
        >
          {t(`modules.${target}`)}
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
