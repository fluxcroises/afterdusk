import { handleMenuShortcut } from "./menu";
import { isMac } from "./platform";
import { useLibrary } from "./store/library";
import { useNavigation } from "./store/navigation";
import type { WindowLabel } from "./windows";

/**
 * Comportements d'une application de bureau plutôt que d'une page web :
 * pas de menu contextuel du navigateur, pas de rechargement ni d'impression
 * de la page. Les raccourcis des menus (F6, F7, F8, Ctrl+Alt+1…) sont portés
 * par la barre de menus (`menu.ts`) ; Tab n'est pas repris de Lightroom, il
 * sert à la navigation au clavier.
 */

/** Raccourcis du navigateur qui n'ont pas de sens dans l'application. */
function isBrowserShortcut(e: KeyboardEvent): boolean {
  const mod = isMac ? e.metaKey : e.ctrlKey;
  const key = e.key.toLowerCase();
  if (e.key === "F5" || e.key === "F3" || e.key === "F12") return true;
  if (mod && ["r", "p", "f", "g", "u", "j", "h", "s"].includes(key)) return true;
  if (mod && e.shiftKey && ["i", "c", "j"].includes(key)) return true;
  return false;
}

function isTyping(target: EventTarget | null): boolean {
  return (
    target instanceof HTMLElement &&
    (target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName))
  );
}

/** Raccourcis sans entrée de menu : flèches pour passer d'une photo à l'autre. */
export function handleShortcut(e: KeyboardEvent): boolean {
  if (isTyping(e.target)) return false;
  const mod = isMac ? e.metaKey : e.ctrlKey;
  const screen = useNavigation.getState().screen;
  if ((screen === "cull" || screen === "library") && !mod && !e.altKey) {
    if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
      useLibrary.getState().move(e.key === "ArrowLeft" ? -1 : 1);
      return true;
    }
  }
  return false;
}

export function installDesktopBehavior(label: WindowLabel): void {
  window.addEventListener("keydown", (e) => {
    const handled =
      label === "main" && !isTyping(e.target) && (handleMenuShortcut(e) || handleShortcut(e));
    if (handled || (!import.meta.env.DEV && isBrowserShortcut(e))) {
      e.preventDefault();
    }
  });
  // Le menu contextuel du navigateur reste disponible en développement (inspecteur).
  if (!import.meta.env.DEV) {
    window.addEventListener("contextmenu", (e) => {
      if (!isTyping(e.target)) e.preventDefault();
    });
  }
}
