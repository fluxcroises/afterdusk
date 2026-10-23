import { invoke, isTauri } from "@tauri-apps/api/core";
import {
  Menu,
  type MenuItemOptions,
  type PredefinedMenuItemOptions,
  type SubmenuOptions,
} from "@tauri-apps/api/menu";
import type { TFunction } from "i18next";
import i18n from "./i18n";
import { KEY_HINTS } from "./menu";
import { isMac } from "./platform";
import { useBursts } from "./store/bursts";
import { useDevelop } from "./store/develop";
import { type MediaSummary, useLibrary } from "./store/library";
import { purgeSelection } from "./trash";
import { useNavigation } from "./store/navigation";
import { pasteTargets } from "./ui/paste";
import { COLOR_LABELS } from "./ui/labels";

type Item = MenuItemOptions | PredefinedMenuItemOptions | SubmenuOptions;

const separator: PredefinedMenuItemOptions = { item: "Separator" };

/**
 * Raccourci affiché à droite de l'entrée, sous Windows (« Retenue\tP ») :
 * une touche seule ne peut pas être un accélérateur, elle bloquerait la
 * saisie. Les menus de macOS n'affichent pas ce texte : rien n'y est ajouté.
 */
function hint(text: string, key: string | undefined): string {
  return key && !isMac ? `${text}\t${key}` : text;
}

/** « Ctrl+Maj+V » : copier et coller les réglages. */
function shortcut(t: TFunction, key: string): string {
  return `${t("keys.ctrl")}+${t("keys.shift")}+${key}`;
}

/** Menu contextuel du système, à la position du pointeur. */
async function popup(items: Item[]): Promise<void> {
  if (!isTauri() || items.length === 0) return;
  const menu = await Menu.new({ items });
  await menu.popup();
}

/** Entrées du clic droit sur une photo (grille, pellicule, visionneuse). */
export function photoMenuItems(t: TFunction, media: MediaSummary): Item[] {
  const library = useLibrary.getState();
  const nav = useNavigation.getState();
  const mediaId = media.id;
  if (library.trash) {
    return [
      { text: t("menu.restore"), action: () => void useLibrary.getState().restore() },
      { text: t("menu.purge"), action: () => void purgeSelection() },
    ];
  }
  const items: Item[] = [
    {
      text: hint(t("context.openLoupe"), t("keys.enter")),
      action: () => {
        if (nav.screen !== "cull" && nav.screen !== "library") nav.goTo("cull");
        useLibrary.getState().setView("loupe");
      },
    },
    {
      text: t("context.develop"),
      action: () => nav.goTo("develop"),
    },
    separator,
    {
      text: t("menu.setRating"),
      items: [0, 1, 2, 3, 4, 5].map((n) => ({
        text: hint(
          n === 0 ? t("menu.noRating") : t("toolbar.stars", { count: n }),
          KEY_HINTS[`rating-${n}`],
        ),
        action: () => void useLibrary.getState().setRating(n),
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
        text: hint(t(label), KEY_HINTS[id]),
        action: () => void useLibrary.getState().setFlag(value),
      })),
    },
    {
      text: t("menu.setColor"),
      items: COLOR_LABELS.map((c) => ({
        text: hint(t(`toolbar.colors.${c}`), KEY_HINTS[`color-${c}`]),
        action: () => void useLibrary.getState().toggleColor(c),
      })),
    },
  ];
  if (useDevelop.getState().clipboard) {
    items.push(separator, {
      text: hint(t("menu.pasteSettings"), shortcut(t, "V")),
      action: () => void useDevelop.getState().pasteSettings(pasteTargets()),
    });
  }
  items.push(separator, {
    text: t("menu.setAlbumCover"),
    action: () => void useLibrary.getState().setAsCover(mediaId),
  });
  if (media.companionOf !== null) {
    items.push({
      text: t("menu.stackTop"),
      action: () => void useLibrary.getState().setStackTop(mediaId),
    });
  }
  if (media.burstId && media.companionOf === null) {
    items.push(
      { text: t("menu.burstTop"), action: () => void useLibrary.getState().setStackTop(mediaId) },
      { text: t("menu.unstackBurst"), action: () => void useBursts.getState().unstackSelection() },
    );
  }
  // Une copie virtuelle n'a pas de fichier : ni Explorateur, ni corbeille.
  if (!media.copyOf) {
    items.push(separator, {
      text: isMac ? t("context.revealFinder") : t("context.revealExplorer"),
      action: () => void invoke("media_reveal", { mediaId }).catch(() => undefined),
    });
    items.push(separator, {
      text: hint(t("menu.moveToTrash"), isMac ? undefined : t("keys.delete")),
      action: () => void useLibrary.getState().moveToTrash(),
    });
    if (media.stackSize > 1 || media.companionOf !== null) {
      items.push(
        {
          text: t("menu.trashJpeg"),
          action: () => void useLibrary.getState().trashFormat("image"),
        },
        { text: t("menu.trashRaw"), action: () => void useLibrary.getState().trashFormat("raw") },
      );
    }
  }
  return items;
}

/**
 * Clic droit sur une photo : elle rejoint la sélection si elle n'y est pas
 * (comme dans Lightroom), puis le menu s'ouvre.
 */
export async function openPhotoMenu(media: MediaSummary): Promise<void> {
  const library = useLibrary.getState();
  const selected = library.selectedIds.includes(media.id) || library.selectedId === media.id;
  if (!selected) await library.select(media.id);
  await popup(photoMenuItems(i18n.t.bind(i18n), media));
}

/** Entrées du clic droit sur l'image du Retoucher. */
export function developMenuItems(t: TFunction): Item[] {
  const develop = useDevelop.getState();
  const items: Item[] = [
    {
      text: hint(t("context.beforeAfter"), "\\"),
      action: () => {
        const d = useDevelop.getState();
        d.setCompare(d.compare === "before" ? "off" : "before");
      },
    },
    {
      text: develop.zoom ? t("develop.zoom.exitHint") : t("develop.zoom.hint"),
      action: () => {
        const d = useDevelop.getState();
        if (d.zoom) d.exitZoom();
        else void d.zoomAt(0.5, 0.5);
      },
    },
  ];
  items.push(
    separator,
    {
      text: hint(t("menu.copySettings"), shortcut(t, "C")),
      enabled: develop.settings !== null,
      action: () => useDevelop.getState().setCopyOpen(true),
    },
    {
      text: hint(t("menu.pasteSettings"), shortcut(t, "V")),
      enabled: develop.clipboard !== null,
      action: () => void useDevelop.getState().pasteSettings(pasteTargets()),
    },
  );
  return items;
}

export async function openDevelopMenu(): Promise<void> {
  await popup(developMenuItems(i18n.t.bind(i18n)));
}
