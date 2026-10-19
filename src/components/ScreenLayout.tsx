import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { useLayout } from "../store/layout";

interface ScreenLayoutProps {
  left?: ReactNode;
  right?: ReactNode;
  /** Barre d'outils sous l'espace de travail. */
  toolbar?: ReactNode;
  /** Bandeau au-dessus de l'espace de travail (barre de filtres), hors défilement. */
  header?: ReactNode;
  /** Pied du panneau droit (bouton d'action principale). */
  rightFooter?: ReactNode;
  filmstrip?: ReactNode;
  children: ReactNode;
}

/** Disposition commune : panneau gauche, espace de travail, panneau droit, pellicule. */
export function ScreenLayout({
  left,
  right,
  toolbar,
  header,
  rightFooter,
  filmstrip,
  children,
}: ScreenLayoutProps) {
  const { t } = useTranslation();
  const visible = useLayout((s) => s.visible);
  // Plein écran de la visionneuse : seule la photo reste.
  const immersive = useLayout((s) => s.immersive);

  return (
    <div className="screen">
      <div className="screen-row">
        {left && visible.left && !immersive && (
          <aside className="side-panel side-panel-left" aria-label={t("panels.left")}>
            <div className="side-panel-scroll">{left}</div>
          </aside>
        )}
        <div className="workspace-column">
          {header && !immersive && <div className="workspace-header">{header}</div>}
          <main className="workspace">{children}</main>
          {toolbar && !immersive && <div className="toolbar">{toolbar}</div>}
        </div>
        {right && visible.right && !immersive && (
          <aside className="side-panel side-panel-right" aria-label={t("panels.right")}>
            <div className="side-panel-scroll">{right}</div>
            {rightFooter && <div className="side-panel-footer">{rightFooter}</div>}
          </aside>
        )}
      </div>
      {filmstrip && visible.filmstrip && !immersive && <div className="filmstrip">{filmstrip}</div>}
    </div>
  );
}
