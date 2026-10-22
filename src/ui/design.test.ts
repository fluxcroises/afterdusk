import { describe, expect, it } from "vitest";
import { cells, cover } from "./design";

describe("cases d'une composition", () => {
  // Mêmes valeurs que le test Rust `cases_selon_le_nombre_et_l_orientation`.
  it("côte à côte sur une page en largeur, l'une sous l'autre en hauteur", () => {
    const c = cells(2, 2, 10);
    expect(c[0][0]).toBeCloseTo(0.05);
    expect(c[0][1]).toBeCloseTo(0.1);
    expect(c[0][2]).toBeCloseTo(0.475);
    expect(c[1][0]).toBeCloseTo(0.525);
    expect(c[1][2]).toBeCloseTo(0.95);
    expect(c[1][3]).toBeCloseTo(0.9);
    const p = cells(2, 0.5, 10);
    expect(p[0][2]).toBeCloseTo(0.9);
    expect(p[0][3]).toBeCloseTo(0.475);
    expect(p[1][1]).toBeCloseTo(0.525);
    expect(cells(3, 1.5, 0)).toHaveLength(3);
    expect(cells(4, 1, 0)[3]).toEqual([0.5, 0.5, 1, 1]);
  });

  it("la photo remplit sa case autour du point choisi, sans en sortir", () => {
    // Photo 3:2 dans une case carrée, point à droite : collée au bord droit.
    const r = cover(1, 1.5, [0.95, 0.5]);
    expect(r[0]).toBeCloseTo(1 / 3);
    expect(r[2]).toBeCloseTo(1);
    expect(r[1]).toBe(0);
    expect(r[3]).toBe(1);
  });
});
