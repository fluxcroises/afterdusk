import { Bandage, Crop, type LucideIcon, SlidersHorizontal, WandSparkles } from "lucide-react";
import { useTranslation } from "react-i18next";
import { type DevelopTool, useDevelop } from "../store/develop";
import { useLayout } from "../store/layout";

const TOOLS: { tool: DevelopTool; icon: LucideIcon; shortcut?: string }[] = [
  { tool: "adjust", icon: SlidersHorizontal },
  { tool: "filters", icon: WandSparkles },
  { tool: "crop", icon: Crop, shortcut: "R" },
  { tool: "spots", icon: Bandage, shortcut: "Q" },
];

/**
 * Barre d'outils verticale à droite du Retoucher, comme Lightroom : un outil
 * à la fois dans le panneau. Un clic sur l'outil actif replie ou rouvre le
 * panneau.
 */
export function DevelopToolRail() {
  const { t } = useTranslation();
  const tool = useDevelop((s) => s.tool);
  const disabled = useDevelop((s) => s.settings === null);
  const panelVisible = useLayout((s) => s.visible.right);
  const togglePanel = useLayout((s) => s.togglePanel);

  return (
    <nav className="tool-rail" aria-label={t("develop.tools.label")}>
      {TOOLS.map(({ tool: id, icon: Icon, shortcut }) => {
        const label = t(`develop.tools.${id}`);
        return (
          <button
            key={id}
            type="button"
            className="tool-rail-button"
            aria-label={label}
            aria-pressed={tool === id}
            title={shortcut ? `${label} (${shortcut})` : label}
            disabled={disabled && id !== "adjust"}
            onClick={() => {
              if (tool === id) {
                togglePanel("right");
                return;
              }
              if (!panelVisible) togglePanel("right");
              void useDevelop.getState().setTool(id);
            }}
          >
            <Icon aria-hidden="true" strokeWidth={1.75} />
          </button>
        );
      })}
    </nav>
  );
}
