/** Champs qui ne prennent pas de texte : curseurs, cases à cocher, boutons. */
const NOT_TEXT = ["range", "checkbox", "radio", "button"];

/** Saisie de texte en cours (un curseur ou une case cochée n'en est pas une). */
export function isTyping(target: EventTarget | null): boolean {
  if (target instanceof HTMLInputElement) return !NOT_TEXT.includes(target.type);
  return (
    target instanceof HTMLElement &&
    (target.isContentEditable || ["TEXTAREA", "SELECT"].includes(target.tagName))
  );
}

/** Espace ou Entrée sur un contrôle qui s'en sert déjà (case, bouton, curseur). */
export function controlKey(e: KeyboardEvent): boolean {
  const t = e.target;
  if (e.key !== " " && e.key !== "Enter") return false;
  return (
    t instanceof HTMLButtonElement || (t instanceof HTMLInputElement && NOT_TEXT.includes(t.type))
  );
}
