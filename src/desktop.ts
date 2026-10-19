import { handleMenuShortcut } from "./menu";
import { isMac } from "./platform";
import { type ColorName, useLibrary } from "./store/library";
import { useNavigation } from "./store/navigation";
import type { WindowLabel } from "./windows";

/**
 * Comportements d'une application de bureau plutôt que d'une page web :
 * pas de menu contextuel du navigateur, pas de rechargement ni d'impression
 * de la page. Les raccourcis des menus (F6, F7, F8, Ctrl+Alt+1…) sont portés
 * par la barre de menus (`menu.ts`), ceux du tri (0 à 9, P, X, U, flèches)
 * par `handleShortcut`. Tab n'est pas repris de Lightroom : il sert à la
 * navigation au clavier.
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

/** Touches 6 à 9 : étiquettes rouge, jaune, vert, bleu (cahier des charges, section 6). */
const COLOR_KEYS: Record<string, ColorName> = {
  "6": "red",
  "7": "yellow",
  "8": "green",
  "9": "blue",
};

/**
 * Chiffre tapé sur la rangée du haut ou le pavé numérique. La touche physique
 * (`code`) fait foi, pour les claviers AZERTY où le chiffre demande Maj ; à
 * défaut de code, le caractère produit.
 */
function digit(e: KeyboardEvent): string | null {
  const match = /^(?:Digit|Numpad)(\d)$/.exec(e.code);
  if (match) return match[1] ?? null;
  return !e.code && /^\d$/.test(e.key) ? e.key : null;
}

/**
 * Raccourcis du tri, à une seule touche : ils n'ont pas d'accélérateur de
 * menu, qui bloquerait la saisie de texte. Actifs dans Trier et Bibliothèque.
 */
export function handleShortcut(e: KeyboardEvent): boolean {
  if (isTyping(e.target)) return false;
  const screen = useNavigation.getState().screen;
  if (screen !== "cull" && screen !== "library") return false;
  if (e.ctrlKey || e.metaKey || e.altKey) return false;

  const library = useLibrary.getState();
  if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
    library.move(e.key === "ArrowLeft" ? -1 : 1);
    return true;
  }
  if (library.selectedId === null) return false;

  const d = digit(e);
  if (d !== null && Number(d) <= 5) {
    void library.setRating(Number(d));
    return true;
  }
  const color = d !== null ? COLOR_KEYS[d] : undefined;
  if (color) {
    void library.toggleColor(color);
    return true;
  }
  switch (e.key.toLowerCase()) {
    case "p":
      void library.setFlag(1);
      return true;
    case "x":
      void library.setFlag(-1);
      return true;
    case "u":
      void library.setFlag(0);
      return true;
    default:
      return false;
  }
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
