import { Camera, Images, Map as MapIcon, Settings, type LucideIcon } from "lucide-react";
import { useTranslation } from "react-i18next";
import { type View, VIEWS, useNavigation } from "../store/navigation";

const ICONS: Record<View, LucideIcon> = {
  library: Images,
  map: MapIcon,
  gear: Camera,
  settings: Settings,
};

export function Sidebar() {
  const { t } = useTranslation();
  const screen = useNavigation((s) => s.screen);
  const goTo = useNavigation((s) => s.goTo);

  return (
    <nav className="sidebar" aria-label={t("views.label")}>
      <ul>
        {VIEWS.map((view) => {
          const Icon = ICONS[view];
          return (
            <li key={view}>
              <button
                type="button"
                className="nav-button"
                aria-current={screen === view ? "page" : undefined}
                onClick={() => goTo(view)}
              >
                <Icon aria-hidden="true" strokeWidth={1.75} />
                {t(`views.${view}`)}
              </button>
            </li>
          );
        })}
      </ul>
    </nav>
  );
}
