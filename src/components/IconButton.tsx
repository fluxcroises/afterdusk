import type { LucideIcon } from "lucide-react";

interface IconButtonProps {
  icon: LucideIcon;
  label: string;
  /** Affiché dans l'infobulle, comme dans les logiciels de bureau. */
  shortcut?: string;
  pressed?: boolean;
  disabled?: boolean;
  onClick?: () => void;
}

export function IconButton({
  icon: Icon,
  label,
  shortcut,
  pressed,
  disabled,
  onClick,
}: IconButtonProps) {
  return (
    <button
      type="button"
      className="icon-button"
      aria-label={label}
      aria-pressed={pressed}
      title={shortcut ? `${label} (${shortcut})` : label}
      disabled={disabled}
      onClick={onClick}
    >
      <Icon aria-hidden="true" strokeWidth={1.75} />
    </button>
  );
}
