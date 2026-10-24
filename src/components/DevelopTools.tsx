import { useAllows } from "../store/profile";
import { Bandage, Blend, Crop, type LucideIcon, SlidersHorizontal, SwatchBook } from "lucide-react";
import { useTranslation } from "react-i18next";
import { type DevelopTool, useDevelop } from "../store/develop";
import { useLayout } from "../store/layout";

const TOOLS: { tool: DevelopTool; icon: LucideIcon; shortcut?: string }[] = [
  { tool: "adjust", icon: SlidersHorizontal },
  // Nuancier : une collection de rendus prêts à appliquer.
  { tool: "filters", icon: SwatchBook },
  { tool: "crop", icon: Crop, shortcut: "R" },
  { tool: "spots", icon: Bandage, shortcut: "Q" },
  { tool: "masks", icon: Blend, shortcut: "M" },
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
  const spots = useAllows("develop.spots");
  const masks = useAllows("develop.masks");

  return (
    <nav className="tool-rail" aria-label={t("develop.tools.label")}>
      {TOOLS.filter((x) => (x.tool !== "spots" || spots) && (x.tool !== "masks" || masks)).map(
        ({ tool: id, icon: Icon, shortcut }) => {
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
        },
      )}
    </nav>
  );
}
