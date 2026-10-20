import { isTauri } from "@tauri-apps/api/core";
import {
  Menu,
  type MenuItemOptions,
  type PredefinedMenuItemOptions,
  type SubmenuOptions,
} from "@tauri-apps/api/menu";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { TFunction } from "i18next";
import { importFromFolder } from "./actions";
import i18n from "./i18n";
import { drawsOwnMenus, isMac } from "./platform";
import { showSelf, showWelcome, toggleSecondary } from "./windows";
import { useAbout } from "./store/about";
import { purgeSelection, trashRejected } from "./trash";
import { focusSearch } from "./search";
import { useLayout } from "./store/layout";
import { pasteTargets } from "./ui/paste";
import { canRedo, canUndo, useDevelop } from "./store/develop";
import { useLibrary } from "./store/library";
import { COLOR_LABELS } from "./ui/labels";
import { STEPS, VIEWS, useNavigation } from "./store/navigation";

type Item = MenuItemOptions | PredefinedMenuItemOptions | SubmenuOptions;

const separator: PredefinedMenuItemOptions = { item: "Separator" };

export type MenuEntry = Item;

/**
 * Touches du tri affichées dans les menus dessinés. Elles ne sont pas des
 * accélérateurs : une touche seule en accélérateur bloquerait la saisie.
 */
export const KEY_HINTS: Record<string, string> = {
  "rating-0": "0",
  "rating-1": "1",
  "rating-2": "2",
  "rating-3": "3",
  "rating-4": "4",
  "rating-5": "5",
  "flag-pick": "P",
  "flag-reject": "X",
  "flag-none": "U",
  "color-red": "6",
  "color-yellow": "7",
  "color-green": "8",
  "color-blue": "9",
  "filter-bar": "\\",
  "stack-toggle": "S",
};

/**
 * Arborescence des menus, construite ici pour être traduite par i18next.
 * macOS l'affiche dans la barre de menus du système ; sous Windows, elle est
 * dessinée dans la barre de titre (`components/MenuBar.tsx`).
 * Les commandes pas encore disponibles sont visibles mais grisées, pour que
 * l'on retrouve d'emblée l'organisation des logiciels de bureau habituels.
 * Les raccourcis à une seule touche (P, X, 1 à 5…) restent gérés par
 * l'interface : en accélérateur de menu, ils bloqueraient la saisie de texte.
 */
export function buildMenuItems(t: TFunction): Item[] {
  const nav = useNavigation.getState();
  const layout = useLayout.getState();
  const preferences: MenuItemOptions = {
    id: "preferences",
    text: t("menu.preferences"),
    accelerator: "CmdOrCtrl+,",
    action: () => nav.goTo("settings"),
  };

  // Boîte « À propos » d'Afterdusk, la même sur Windows et macOS (et non le
  // panneau standard du système).
  const about: MenuItemOptions = {
    id: "about",
    text: t("menu.about"),
    action: () => {
      void showSelf();
      useAbout.getState().show();
    },
  };

  const appMenu: SubmenuOptions = {
    text: t("menu.app"),
    items: [
      about,
      separator,
      preferences,
      separator,
      { item: "Hide", text: t("menu.hide") },
      { item: "HideOthers", text: t("menu.hideOthers") },
      separator,
      { item: "Quit", text: t("menu.quit") },
    ],
  };

  const file: SubmenuOptions = {
    text: t("menu.file"),
    items: [
      {
        id: "import-device",
        text: t("menu.importDevice"),
        accelerator: "CmdOrCtrl+Shift+I",
        action: () => nav.goTo("import"),
      },
      {
        id: "import-folder",
        text: t("menu.importFolder"),
        accelerator: "CmdOrCtrl+Shift+O",
        action: () => void importFromFolder(),
      },
      separator,
      { id: "new-library", text: t("menu.newLibrary"), enabled: false },
      separator,
      { item: "CloseWindow", text: t("menu.closeWindow") },
      ...(isMac ? [] : [{ item: "Quit", text: t("menu.quit") } as PredefinedMenuItemOptions]),
    ],
  };

  const inDevelop = () => useNavigation.getState().screen === "develop";
  const edit: SubmenuOptions = {
    text: t("menu.edit"),
    items: [
      {
        id: "undo",
        // Dans le Retoucher : l'historique de la photo ; ailleurs, le dernier
        // envoi dans la corbeille.
        text: t("menu.undo"),
        accelerator: "CmdOrCtrl+Z",
        enabled:
          isMac ||
          (inDevelop() ? canUndo(useDevelop.getState()) : useLibrary.getState().undoIds !== null),
        action: () => {
          if (inDevelop()) void useDevelop.getState().undo();
          else void useLibrary.getState().undoTrash();
        },
      },
      {
        id: "redo",
        text: t("menu.redo"),
        accelerator: "CmdOrCtrl+Shift+Z",
        enabled: isMac || (inDevelop() && canRedo(useDevelop.getState())),
        action: () => {
          if (inDevelop()) void useDevelop.getState().redo();
        },
      },
      separator,
      {
        id: "copy-settings",
        text: t("menu.copySettings"),
        accelerator: "CmdOrCtrl+Shift+C",
        enabled: isMac || inDevelop(),
        action: () => {
          if (inDevelop() && useDevelop.getState().settings) {
            useDevelop.getState().setCopyOpen(true);
          }
        },
      },
      {
        id: "paste-settings",
        text: t("menu.pasteSettings"),
        accelerator: "CmdOrCtrl+Shift+V",
        enabled: isMac || inDevelop(),
        action: () => {
          if (inDevelop()) void useDevelop.getState().pasteSettings(pasteTargets());
        },
      },
      separator,
      { item: "Cut", text: t("menu.cut") },
      { item: "Copy", text: t("menu.copy") },
      { item: "Paste", text: t("menu.paste") },
      separator,
      { item: "SelectAll", text: t("menu.selectAll") },
      {
        id: "find",
        text: t("menu.find"),
        accelerator: "CmdOrCtrl+F",
        action: () => {
          const screen = useNavigation.getState().screen;
          if (screen === "cull" || screen === "library") focusSearch();
        },
      },
      { id: "select-none", text: t("menu.selectNone"), enabled: false },
      ...(isMac ? [] : [separator, preferences]),
    ],
  };

  // Le tri porte sur la photo sélectionnée, dans Trier ou Bibliothèque. Le menu
  // natif de macOS est construit une fois : ses entrées restent actives et la
  // condition est vérifiée au moment de l'action.
  const onPhotos = () => {
    const screen = useNavigation.getState().screen;
    return screen === "cull" || screen === "library";
  };
  const canMarkNow = () => {
    const { selectedId, trash } = useLibrary.getState();
    return onPhotos() && selectedId !== null && !trash;
  };
  const canMark = isMac || canMarkNow();
  const whenMarkable = (run: () => void) => () => {
    if (canMarkNow()) run();
  };
  // Dans la vue Corbeille : restaurer ou supprimer définitivement.
  const inTrashNow = () => {
    const { selectedId, trash } = useLibrary.getState();
    return onPhotos() && selectedId !== null && trash;
  };
  const inTrash = isMac || inTrashNow();
  const whenInTrash = (run: () => void) => () => {
    if (inTrashNow()) run();
  };
  const canTrashRejectedNow = () => onPhotos() && !useLibrary.getState().trash;
  const photo: SubmenuOptions = {
    text: t("menu.photo"),
    items: [
      {
        text: t("menu.setRating"),
        items: [0, 1, 2, 3, 4, 5].map((n) => ({
          id: "rating-" + n,
          text: n === 0 ? t("menu.noRating") : t("toolbar.stars", { count: n }),
          enabled: canMark,
          action: whenMarkable(() => void useLibrary.getState().setRating(n)),
        })),
      },
      {
        text: t("menu.setFlag"),
        items: (
          [
            ["flag-pick", "toolbar.pick", 1],
            ["flag-reject", "toolbar.reject", -1],
            ["flag-none", "menu.noFlag", 0],
          ] as const
        ).map(([id, label, value]) => ({
          id,
          text: t(label),
          enabled: canMark,
          action: whenMarkable(() => void useLibrary.getState().setFlag(value)),
        })),
      },
      {
        text: t("menu.setColor"),
        items: COLOR_LABELS.map((c) => ({
          id: "color-" + c,
          text: t("toolbar.colors." + c),
          enabled: canMark,
          action: whenMarkable(() => void useLibrary.getState().toggleColor(c)),
        })),
      },
      separator,
      {
        id: "album-cover",
        text: t("menu.setAlbumCover"),
        enabled: canMark,
        action: whenMarkable(() => {
          const id = useLibrary.getState().selectedId;
          if (id !== null) void useLibrary.getState().setAsCover(id);
        }),
      },
      separator,
      {
        id: "stack-toggle",
        text: t("menu.stackToggle"),
        enabled: canMark,
        action: whenMarkable(() => void useLibrary.getState().toggleStack()),
      },
      {
        id: "stack-top",
        text: t("menu.stackTop"),
        enabled: canMark,
        action: whenMarkable(() => void useLibrary.getState().setStackTop()),
      },
      separator,
      {
        id: "trash",
        text: t("menu.moveToTrash"),
        // Suppr comme Lightroom ; ⌘⌫ comme le Finder.
        accelerator: isMac ? "CmdOrCtrl+Backspace" : "Delete",
        enabled: canMark,
        action: whenMarkable(() => void useLibrary.getState().moveToTrash()),
      },
      {
        id: "trash-jpeg",
        text: t("menu.trashJpeg"),
        enabled: canMark,
        action: whenMarkable(() => void useLibrary.getState().trashFormat("image")),
      },
      {
        id: "trash-raw",
        text: t("menu.trashRaw"),
        enabled: canMark,
        action: whenMarkable(() => void useLibrary.getState().trashFormat("raw")),
      },
      {
        id: "trash-rejected",
        text: t("menu.moveRejectedToTrash"),
        accelerator: isMac ? "CmdOrCtrl+Alt+Backspace" : "CmdOrCtrl+Backspace",
        enabled: isMac || canTrashRejectedNow(),
        action: () => {
          if (canTrashRejectedNow()) void trashRejected();
        },
      },
      separator,
      {
        id: "restore",
        text: t("menu.restore"),
        enabled: inTrash,
        action: whenInTrash(() => void useLibrary.getState().restore()),
      },
      {
        id: "purge",
        text: t("menu.purge"),
        enabled: inTrash,
        action: whenInTrash(() => void purgeSelection()),
      },
    ],
  };

  const modules = [...STEPS, ...VIEWS];
  const view: SubmenuOptions = {
    text: t("menu.view"),
    items: [
      {
        text: t("menu.goTo"),
        items: modules.map((screen, index) => ({
          id: `go-${screen}`,
          text: t(`modules.${screen}`),
          accelerator: `CmdOrCtrl+Alt+${index + 1}`,
          action: () => nav.goTo(screen),
        })),
      },
      separator,
      {
        id: "toggle-left",
        text: t("menu.toggleLeft"),
        accelerator: "F7",
        action: () => layout.togglePanel("left"),
      },
      {
        id: "toggle-right",
        text: t("menu.toggleRight"),
        accelerator: "F8",
        action: () => layout.togglePanel("right"),
      },
      {
        id: "filter-bar",
        text: t("menu.filterBar"),
        action: () => {
          const { filterBar, setFilterBar } = useLayout.getState();
          setFilterBar(!filterBar);
        },
      },
      {
        id: "toggle-filmstrip",
        text: t("menu.toggleFilmstrip"),
        accelerator: "F6",
        action: () => layout.togglePanel("filmstrip"),
      },
      separator,
      {
        id: "fullscreen",
        text: t("menu.fullscreen"),
        accelerator: isMac ? "Ctrl+Cmd+F" : "F11",
        action: () => void toggleFullscreen(),
      },
    ],
  };

  const help: SubmenuOptions = {
    text: t("menu.help"),
    items: [
      { id: "shortcuts", text: t("menu.shortcuts"), enabled: false },
      ...(isMac ? [] : [separator, about]),
    ],
  };

  const windowMenu: SubmenuOptions = {
    text: t("menu.window"),
    items: [
      {
        id: "welcome",
        text: t("menu.welcome"),
        accelerator: "CmdOrCtrl+Alt+0",
        action: () => void showWelcome(),
      },
      {
        id: "secondary",
        text: t("menu.secondary"),
        accelerator: "CmdOrCtrl+F11",
        action: () => void toggleSecondary(),
      },
      separator,
      { item: "Minimize", text: t("menu.minimize") },
    ],
  };

  return isMac
    ? [appMenu, file, edit, photo, view, windowMenu, help]
    : [file, edit, photo, view, windowMenu, help];
}

async function toggleFullscreen(): Promise<void> {
  const win = getCurrentWindow();
  await win.setFullscreen(!(await win.isFullscreen()));
}

async function applyMenu(): Promise<void> {
  const menu = await Menu.new({ items: buildMenuItems(i18n.t) });
  await menu.setAsAppMenu();
}

/** macOS : barre de menus du système, reconstruite à chaque changement de langue. */
export function installMenu(): void {
  if (!isTauri() || !isMac) return;
  const apply = () => {
    applyMenu().catch((error: unknown) => console.error("menu", error));
  };
  apply();
  i18n.on("languageChanged", apply);
}

/* ---------- Menus dessinés (Windows) ---------- */

export function isSubmenu(entry: Item): entry is SubmenuOptions {
  return "items" in entry && Array.isArray(entry.items);
}

/** Entrées d'un sous-menu ; buildMenuItems n'y met que des options simples. */
export function children(entry: SubmenuOptions): Item[] {
  return (entry.items ?? []) as Item[];
}

export function isPredefined(entry: Item): entry is PredefinedMenuItemOptions {
  return "item" in entry;
}

export function isSeparator(entry: Item): boolean {
  return isPredefined(entry) && entry.item === "Separator";
}

/** Exécute une entrée prédéfinie (presse-papiers, fenêtre, à propos). */
export async function runPredefined(entry: PredefinedMenuItemOptions): Promise<void> {
  const item = entry.item;
  if (typeof item === "object") {
    useAbout.getState().show();
    return;
  }
  const win = getCurrentWindow();
  switch (item) {
    case "Cut":
    case "Copy":
    case "Paste":
      document.execCommand(item.toLowerCase());
      return;
    case "SelectAll":
      document.execCommand("selectAll");
      return;
    case "Minimize":
      return win.minimize();
    case "CloseWindow":
    case "Quit":
      return win.close();
    default:
      return;
  }
}

/** Raccourci d'une entrée prédéfinie, pour l'affichage. */
export function predefinedAccelerator(entry: PredefinedMenuItemOptions): string | undefined {
  switch (entry.item) {
    case "Cut":
      return "CmdOrCtrl+X";
    case "Copy":
      return "CmdOrCtrl+C";
    case "Paste":
      return "CmdOrCtrl+V";
    case "SelectAll":
      return "CmdOrCtrl+A";
    case "CloseWindow":
      return "CmdOrCtrl+W";
    case "Quit":
      return "Alt+F4";
    default:
      return undefined;
  }
}

/** « CmdOrCtrl+Shift+I » → « Ctrl+Maj+I » (libellés de touches traduits). */
export function formatAccelerator(accelerator: string, t: TFunction): string {
  return accelerator
    .split("+")
    .map((part) => {
      switch (part) {
        case "CmdOrCtrl":
        case "CommandOrControl":
          return isMac ? "⌘" : t("keys.ctrl");
        case "Shift":
          return t("keys.shift");
        case "Alt":
          return t("keys.alt");
        case "Delete":
          return t("keys.delete");
        case "Backspace":
          return isMac ? "⌫" : t("keys.backspace");
        default:
          return part;
      }
    })
    .join("+");
}

/** L'événement clavier correspond-il au raccourci ? */
export function matchesAccelerator(accelerator: string, e: KeyboardEvent): boolean {
  const parts = accelerator.split("+");
  const key = parts.pop() ?? "";
  const wants = (name: string) => parts.includes(name);
  const ctrl = wants("CmdOrCtrl") || wants("CommandOrControl") || wants("Ctrl");
  if (ctrl !== (isMac ? e.metaKey : e.ctrlKey)) return false;
  if (wants("Shift") !== e.shiftKey || wants("Alt") !== e.altKey) return false;
  if (/^F\d+$/.test(key)) return e.key === key;
  if (/^\d$/.test(key)) return e.code === `Digit${key}`;
  if (key === ",") return e.key === ",";
  if (key === "Delete" || key === "Backspace") return e.key === key;
  // La lettre tapée, comme les menus natifs (Ctrl+Z en AZERTY est la touche
  // KeyW) ; la position de la touche pour les claviers non latins.
  const letter = e.key.length === 1 ? e.key.toUpperCase() : "";
  if (/^[A-Z]$/.test(letter)) return letter === key.toUpperCase();
  return e.code === `Key${key.toUpperCase()}`;
}

function* actionable(items: Item[]): Generator<MenuItemOptions> {
  for (const entry of items) {
    if (isSubmenu(entry)) yield* actionable(children(entry));
    else if (!isPredefined(entry)) yield entry as MenuItemOptions;
  }
}

/**
 * Raccourcis des menus dessinés : sans menu natif, c'est l'interface qui les
 * reconnaît. Renvoie vrai si une commande a été exécutée.
 */
export function handleMenuShortcut(e: KeyboardEvent): boolean {
  if (!drawsOwnMenus) return false;
  for (const entry of actionable(buildMenuItems(i18n.t))) {
    if (entry.enabled === false || !entry.accelerator || !entry.action) continue;
    if (matchesAccelerator(entry.accelerator, e)) {
      entry.action(entry.id ?? "");
      return true;
    }
  }
  return false;
}
