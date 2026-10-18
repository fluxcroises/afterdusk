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
import { useLayout } from "./store/layout";
import { STEPS, VIEWS, useNavigation } from "./store/navigation";

type Item = MenuItemOptions | PredefinedMenuItemOptions | SubmenuOptions;

const isMac = navigator.userAgent.includes("Mac");
const separator: PredefinedMenuItemOptions = { item: "Separator" };

/**
 * Barre de menus native, construite ici pour être traduite par i18next.
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

  const appMenu: SubmenuOptions = {
    text: t("menu.app"),
    items: [
      { item: { About: null }, text: t("menu.about") },
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

  const edit: SubmenuOptions = {
    text: t("menu.edit"),
    items: [
      { id: "undo", text: t("menu.undo"), accelerator: "CmdOrCtrl+Z", enabled: false },
      { id: "redo", text: t("menu.redo"), accelerator: "CmdOrCtrl+Shift+Z", enabled: false },
      separator,
      { item: "Cut", text: t("menu.cut") },
      { item: "Copy", text: t("menu.copy") },
      { item: "Paste", text: t("menu.paste") },
      separator,
      { item: "SelectAll", text: t("menu.selectAll") },
      { id: "select-none", text: t("menu.selectNone"), enabled: false },
      ...(isMac ? [] : [separator, preferences]),
    ],
  };

  const photo: SubmenuOptions = {
    text: t("menu.photo"),
    items: [
      { id: "rating", text: t("menu.setRating"), enabled: false },
      { id: "flag", text: t("menu.setFlag"), enabled: false },
      { id: "color", text: t("menu.setColor"), enabled: false },
      separator,
      { id: "trash", text: t("menu.moveToTrash"), enabled: false },
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
      ...(isMac ? [] : [separator, { item: { About: null }, text: t("menu.about") }]),
    ],
  };

  const windowMenu: SubmenuOptions = {
    text: t("menu.window"),
    items: [{ item: "Minimize", text: t("menu.minimize") }],
  };

  return isMac
    ? [appMenu, file, edit, photo, view, windowMenu, help]
    : [file, edit, photo, view, help];
}

async function toggleFullscreen(): Promise<void> {
  const win = getCurrentWindow();
  await win.setFullscreen(!(await win.isFullscreen()));
}

async function applyMenu(): Promise<void> {
  const menu = await Menu.new({ items: buildMenuItems(i18n.t) });
  await menu.setAsAppMenu();
}

/** Installe la barre de menus et la reconstruit à chaque changement de langue. */
export function installMenu(): void {
  if (!isTauri()) return;
  const apply = () => {
    applyMenu().catch((error: unknown) => console.error("menu", error));
  };
  apply();
  i18n.on("languageChanged", apply);
}
