import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";

interface EmptyStateProps {
  icon: LucideIcon;
  title: string;
  children?: ReactNode;
}

/** Message centré dans l'espace de travail quand il n'y a rien à afficher. */
export function EmptyState({ icon: Icon, title, children }: EmptyStateProps) {
  return (
    <div className="empty-state">
      <Icon className="empty-state-icon" aria-hidden="true" strokeWidth={1.25} />
      <h1 className="empty-state-title">{title}</h1>
      {children && <div className="empty-state-text">{children}</div>}
    </div>
  );
}

/** Touche de raccourci affichée dans un texte. */
export function Kbd({ children }: { children: ReactNode }) {
  return <kbd className="kbd">{children}</kbd>;
}
