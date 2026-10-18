import type { LucideIcon } from "lucide-react";
import { useTranslation } from "react-i18next";
import { AVAILABLE_IN, type Screen } from "../store/navigation";
import { EmptyState } from "./EmptyState";
import { ScreenLayout } from "./ScreenLayout";

/** Module prévu dans une version ultérieure : même cadre, espace de travail vide. */
export function UpcomingScreen({ screen, icon }: { screen: Screen; icon: LucideIcon }) {
  const { t } = useTranslation();
  return (
    <ScreenLayout>
      <EmptyState
        icon={icon}
        title={t("upcoming.title", {
          module: t(`modules.${screen}`),
          version: AVAILABLE_IN[screen],
        })}
      >
        {t(`upcoming.${screen}`)}
      </EmptyState>
    </ScreenLayout>
  );
}
