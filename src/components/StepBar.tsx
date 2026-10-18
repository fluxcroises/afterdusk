import { useTranslation } from "react-i18next";
import { STEPS, useNavigation } from "../store/navigation";

export function StepBar() {
  const { t } = useTranslation();
  const screen = useNavigation((s) => s.screen);
  const goTo = useNavigation((s) => s.goTo);

  return (
    <nav className="step-bar" aria-label={t("steps.label")}>
      <ol>
        {STEPS.map((step, index) => (
          <li key={step}>
            <button
              type="button"
              className="nav-button"
              aria-current={screen === step ? "step" : undefined}
              onClick={() => goTo(step)}
            >
              <span className="step-index">{index + 1}</span>
              {t(`steps.${step}`)}
            </button>
          </li>
        ))}
      </ol>
    </nav>
  );
}
