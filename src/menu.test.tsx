import { describe, expect, it } from "vitest";
import i18n from "./i18n";
import { fireEvent, render, screen } from "@testing-library/react";
import { MenuBar } from "./components/MenuBar";
import { buildMenuItems, formatAccelerator, handleMenuShortcut, matchesAccelerator } from "./menu";
import { useNavigation } from "./store/navigation";

type Node = { text?: string; items?: Node[]; accelerator?: string };

function walk(items: Node[], visit: (n: Node) => void) {
  for (const item of items) {
    visit(item);
    if (item.items) walk(item.items, visit);
  }
}

describe("barre de menus", () => {
  it("suit l'ordre habituel des logiciels de bureau (Windows)", async () => {
    await i18n.changeLanguage("fr");
    const top = buildMenuItems(i18n.t) as Node[];
    expect(top.map((m) => m.text)).toEqual([
      "Fichier",
      "Édition",
      "Photo",
      "Affichage",
      "Fenêtre",
      "Aide",
    ]);
  });

  it("n'a aucun libellé non traduit ni raccourci en double", async () => {
    for (const lng of ["fr", "en"]) {
      await i18n.changeLanguage(lng);
      const seen = new Set<string>();
      walk(buildMenuItems(i18n.t) as Node[], (n) => {
        if (n.text) expect(n.text).not.toMatch(/^menu\.|^modules\./);
        if (n.accelerator) {
          expect(seen.has(n.accelerator), n.accelerator).toBe(false);
          seen.add(n.accelerator);
        }
      });
    }
  });

  it("n'utilise pas de raccourci à une seule touche, qui bloquerait la saisie", () => {
    walk(buildMenuItems(i18n.t) as Node[], (n) => {
      if (n.accelerator && !/^F\d+$/.test(n.accelerator)) {
        expect(n.accelerator).toContain("+");
      }
    });
  });
});

describe("menus dessinés (Windows)", () => {
  it("reconnaît les raccourcis et les affiche avec les noms de touches traduits", async () => {
    await i18n.changeLanguage("fr");
    const e = (init: KeyboardEventInit) => new KeyboardEvent("keydown", init);
    expect(
      matchesAccelerator("CmdOrCtrl+Shift+I", e({ code: "KeyI", ctrlKey: true, shiftKey: true })),
    ).toBe(true);
    expect(matchesAccelerator("CmdOrCtrl+Shift+I", e({ code: "KeyI", ctrlKey: true }))).toBe(false);
    expect(
      matchesAccelerator("CmdOrCtrl+Alt+2", e({ code: "Digit2", ctrlKey: true, altKey: true })),
    ).toBe(true);
    expect(matchesAccelerator("F7", e({ key: "F7" }))).toBe(true);
    expect(formatAccelerator("CmdOrCtrl+Shift+I", i18n.t)).toBe("Ctrl+Maj+I");
  });

  it("Ctrl+Alt+2 ouvre Trier", () => {
    useNavigation.setState({ screen: "import" });
    const handled = handleMenuShortcut(
      new KeyboardEvent("keydown", { code: "Digit2", ctrlKey: true, altKey: true }),
    );
    expect(handled).toBe(true);
    expect(useNavigation.getState().screen).toBe("cull");
  });

  it("ouvre un menu au clic et exécute une commande", async () => {
    await i18n.changeLanguage("fr");
    useNavigation.setState({ screen: "import" });
    render(<MenuBar />);
    fireEvent.click(screen.getByRole("menuitem", { name: "Affichage" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Aller à" }));
    fireEvent.click(screen.getByRole("menuitem", { name: /Bibliothèque/ }));
    expect(useNavigation.getState().screen).toBe("library");
    // Le menu se referme après la commande.
    expect(screen.queryByRole("menu")).toBeNull();
  });
});
