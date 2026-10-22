import { useProfile } from "../store/profile";
import type { MenuItemOptions, SubmenuOptions } from "@tauri-apps/api/menu";
import { ChevronRight } from "lucide-react";
import { type KeyboardEvent, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  type MenuEntry,
  buildMenuItems,
  children,
  KEY_HINTS,
  formatAccelerator,
  isPredefined,
  isSeparator,
  isSubmenu,
  predefinedAccelerator,
  runPredefined,
} from "../menu";

interface MenuListProps {
  items: MenuEntry[];
  onDone: () => void;
}

/** Liste déroulante d'un menu, avec sous-menus au survol. */
function MenuList({ items, onDone }: MenuListProps) {
  const { t } = useTranslation();
  const [submenu, setSubmenu] = useState<number | null>(null);

  const moveFocus = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    e.stopPropagation();
    const list = e.currentTarget;
    const rows = [...list.querySelectorAll<HTMLButtonElement>(":scope > .menu-row:not(:disabled)")];
    const index = rows.indexOf(document.activeElement as HTMLButtonElement);
    const next = e.key === "ArrowDown" ? index + 1 : index - 1;
    rows[(next + rows.length) % rows.length]?.focus();
  };

  return (
    <div className="menu-list" role="menu" onKeyDown={moveFocus}>
      {items.map((entry, index) => {
        if (isSeparator(entry))
          return <div key={index} className="menu-separator" role="separator" />;
        if (isSubmenu(entry)) {
          const sub = entry as SubmenuOptions;
          return (
            <div key={index} className="menu-sub" onMouseEnter={() => setSubmenu(index)}>
              <button
                type="button"
                className="menu-row"
                role="menuitem"
                aria-haspopup="menu"
                aria-expanded={submenu === index}
                onClick={() => setSubmenu(index)}
              >
                <span className="menu-label">{sub.text}</span>
                <ChevronRight className="menu-chevron" aria-hidden="true" />
              </button>
              {submenu === index && <MenuList items={children(sub)} onDone={onDone} />}
            </div>
          );
        }
        if (isPredefined(entry)) {
          const accelerator = predefinedAccelerator(entry);
          return (
            <button
              key={index}
              type="button"
              className="menu-row"
              role="menuitem"
              onMouseEnter={() => setSubmenu(null)}
              onClick={() => {
                onDone();
                void runPredefined(entry);
              }}
            >
              <span className="menu-label">{entry.text}</span>
              {accelerator && (
                <span className="menu-accelerator">{formatAccelerator(accelerator, t)}</span>
              )}
            </button>
          );
        }
        const item = entry as MenuItemOptions;
        return (
          <button
            key={index}
            type="button"
            className="menu-row"
            role="menuitem"
            disabled={item.enabled === false}
            onMouseEnter={() => setSubmenu(null)}
            onClick={() => {
              onDone();
              item.action?.(item.id ?? "");
            }}
          >
            <span className="menu-label">{item.text}</span>
            {item.accelerator ? (
              <span className="menu-accelerator">{formatAccelerator(item.accelerator, t)}</span>
            ) : (
              item.id &&
              KEY_HINTS[item.id] && <span className="menu-accelerator">{KEY_HINTS[item.id]}</span>
            )}
          </button>
        );
      })}
    </div>
  );
}

/**
 * Barre de menus dessinée dans la barre de titre (Windows), avec le
 * comportement d'une barre native : un clic ouvre, le survol passe d'un menu
 * à l'autre, Échap ou un clic ailleurs ferment, Alt donne le focus.
 */
export function MenuBar() {
  const { t } = useTranslation();
  const [open, setOpen] = useState<number | null>(null);
  const bar = useRef<HTMLDivElement>(null);
  // Reconstruits aussi quand le profil d'interface change.
  useProfile((s) => s.profile);
  const menus = buildMenuItems(t).filter(isSubmenu) as SubmenuOptions[];

  useEffect(() => {
    const close = (e: MouseEvent) => {
      if (!bar.current?.contains(e.target as Node)) setOpen(null);
    };
    let altAlone = false;
    const onKeyDown = (e: globalThis.KeyboardEvent) => {
      altAlone = e.key === "Alt" && !e.ctrlKey && !e.shiftKey;
      if (e.key === "Escape") setOpen(null);
    };
    // Alt seul, comme sous Windows : donne le focus au premier menu.
    const onKeyUp = (e: globalThis.KeyboardEvent) => {
      if (e.key === "Alt" && altAlone) {
        e.preventDefault();
        bar.current?.querySelector<HTMLButtonElement>(".menu-title")?.focus();
      }
      altAlone = false;
    };
    window.addEventListener("mousedown", close);
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("keyup", onKeyUp);
    window.addEventListener("blur", () => setOpen(null));
    return () => {
      window.removeEventListener("mousedown", close);
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("keyup", onKeyUp);
    };
  }, []);

  const switchMenu = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
    const titles = [...(bar.current?.querySelectorAll<HTMLButtonElement>(".menu-title") ?? [])];
    const current = open ?? titles.indexOf(document.activeElement as HTMLButtonElement);
    if (current < 0) return;
    e.preventDefault();
    const next = (current + (e.key === "ArrowRight" ? 1 : -1) + titles.length) % titles.length;
    titles[next]?.focus();
    if (open !== null) setOpen(next);
  };

  return (
    <div
      className="menu-bar"
      role="menubar"
      aria-label={t("menu.menuBar")}
      ref={bar}
      onKeyDown={switchMenu}
    >
      {menus.map((menu, index) => (
        <div key={menu.text} className="menu">
          <button
            type="button"
            className="menu-title"
            role="menuitem"
            aria-haspopup="menu"
            aria-expanded={open === index}
            onClick={() => setOpen(open === index ? null : index)}
            onMouseEnter={() => open !== null && setOpen(index)}
            onKeyDown={(e) => {
              if (e.key === "ArrowDown" || e.key === "Enter") {
                e.preventDefault();
                setOpen(index);
                requestAnimationFrame(() =>
                  bar.current
                    ?.querySelector<HTMLButtonElement>(".menu-list .menu-row:not(:disabled)")
                    ?.focus(),
                );
              }
            }}
          >
            {menu.text}
          </button>
          {open === index && <MenuList items={children(menu)} onDone={() => setOpen(null)} />}
        </div>
      ))}
    </div>
  );
}
