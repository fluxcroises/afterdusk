import { ChevronDown } from "lucide-react";
import type { ReactNode } from "react";
import { useLayout } from "../store/layout";

interface PanelSectionProps {
  /** Identifiant stable, pour mémoriser l'état replié. */
  id: string;
  title: string;
  /** Contenu discret à droite du titre (compteur, bouton). */
  aside?: ReactNode;
  children: ReactNode;
}

/** Section repliable d'un panneau latéral. */
export function PanelSection({ id, title, aside, children }: PanelSectionProps) {
  const collapsed = useLayout((s) => s.collapsed[id] ?? false);
  const toggle = useLayout((s) => s.toggleSection);
  const bodyId = `section-${id}`;

  return (
    <section className="panel-section">
      <h2 className="panel-section-header">
        <button
          type="button"
          aria-expanded={!collapsed}
          aria-controls={bodyId}
          onClick={() => toggle(id)}
        >
          <ChevronDown className="chevron" aria-hidden="true" strokeWidth={2} />
          {title}
        </button>
        {aside}
      </h2>
      {!collapsed && (
        <div className="panel-section-body" id={bodyId}>
          {children}
        </div>
      )}
    </section>
  );
}

interface ListRowProps {
  label: string;
  count?: number;
  selected?: boolean;
  onClick?: () => void;
}

/** Ligne de navigation d'un panneau (catalogue, album, collection). */
export function ListRow({ label, count, selected, onClick }: ListRowProps) {
  return (
    <button
      type="button"
      className="list-row"
      aria-current={selected || undefined}
      onClick={onClick}
    >
      <span className="list-row-label">{label}</span>
      {count !== undefined && <span className="list-row-count">{count}</span>}
    </button>
  );
}

interface FieldRowProps {
  label: string;
  value?: ReactNode;
}

/** Libellé à gauche, valeur à droite (fiche d'informations, options d'import). */
export function FieldRow({ label, value }: FieldRowProps) {
  return (
    <div className="field-row">
      <span className="field-row-label">{label}</span>
      <span className="field-row-value">{value ?? "—"}</span>
    </div>
  );
}

export function PanelNote({ children }: { children: ReactNode }) {
  return <p className="panel-note">{children}</p>;
}
