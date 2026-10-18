import { isTauri } from "@tauri-apps/api/core";
import { useLayout } from "./store/layout";
import { useLibrary } from "./store/library";
import { STEPS, VIEWS, useNavigation } from "./store/navigation";

/**
 * Comportements d'une application de bureau plutôt que d'une page web :
 * pas de menu contextuel du navigateur, pas de rechargement ni d'impression
 * de la page, et les raccourcis de panneaux de Lightroom (F6, F7, F8).
 * Tab n'est pas repris de Lightroom : il sert à la navigation au clavier.
 * Dans l'application, ces raccourcis sont portés par la barre de menus
 * (menu.ts) ; ce gestionnaire ne sert alors qu'aux combinaisons absentes du menu.
 */

const isMac = navigator.userAgent.includes("Mac");

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

export function handleShortcut(e: KeyboardEvent): boolean {
  if (isTyping(e.target)) return false;
  const layout = useLayout.getState();
  const mod = isMac ? e.metaKey : e.ctrlKey;

  // Flèches : photo précédente ou suivante dans Trier et Bibliothèque.
  const screen = useNavigation.getState().screen;
  if ((screen === "cull" || screen === "library") && !mod && !e.altKey) {
    if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
      useLibrary.getState().move(e.key === "ArrowLeft" ? -1 : 1);
      return true;
    }
  }

  // Dans l'application, les raccourcis suivants passent par la barre de menus.
  if (isTauri()) return false;

  if (e.key === "F7") {
    layout.togglePanel("left");
    return true;
  }
  if (e.key === "F8") {
    layout.togglePanel("right");
    return true;
  }
  if (e.key === "F6") {
    layout.togglePanel("filmstrip");
    return true;
  }
  // Ctrl+Alt+1… : changer de module, comme dans Lightroom.
  if (mod && e.altKey && /^Digit[1-7]$/.test(e.code)) {
    const screens = [...STEPS, ...VIEWS];
    const target = screens[Number(e.code.slice(5)) - 1];
    if (target) useNavigation.getState().goTo(target);
    return true;
  }
  if (mod && e.key === ",") {
    useNavigation.getState().goTo("settings");
    return true;
  }
  return false;
}

export function installDesktopBehavior(): void {
  window.addEventListener("keydown", (e) => {
    if (handleShortcut(e) || (!import.meta.env.DEV && isBrowserShortcut(e))) {
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
