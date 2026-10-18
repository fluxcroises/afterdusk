import { describe, expect, it } from "vitest";
import i18n from "./i18n";
import { buildMenuItems } from "./menu";

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
