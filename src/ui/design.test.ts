import { describe, expect, it } from "vitest";
import { POLAROID_CELL, STRIP_BAND, cells, columns, cover, inkFor } from "./design";

describe("cases d'une composition", () => {
  // Mêmes valeurs que le test Rust `cases_selon_le_nombre_et_l_orientation`.
  it("côte à côte sur une page en largeur, l'une sous l'autre en hauteur", () => {
    const c = cells("grid", 2, 2, 10);
    expect(c[0][0]).toBeCloseTo(0.05);
    expect(c[0][1]).toBeCloseTo(0.1);
    expect(c[0][2]).toBeCloseTo(0.475);
    expect(c[1][0]).toBeCloseTo(0.525);
    expect(c[1][2]).toBeCloseTo(0.95);
    expect(c[1][3]).toBeCloseTo(0.9);
    const p = cells("grid", 2, 0.5, 10);
    expect(p[0][2]).toBeCloseTo(0.9);
    expect(p[0][3]).toBeCloseTo(0.475);
    expect(p[1][1]).toBeCloseTo(0.525);
    expect(cells("grid", 3, 1.5, 0)).toHaveLength(3);
    expect(cells("grid", 4, 1, 0)[3]).toEqual([0.5, 0.5, 1, 1]);
  });

  // Mêmes valeurs que le test Rust `bande_et_polaroid`.
  it("bande photomaton, doublée, et Polaroid", () => {
    const c = cells("strip", 4, 1 / 3, 0);
    expect(c).toHaveLength(4);
    expect(c[3][3]).toBeCloseTo(1 - STRIP_BAND);
    expect(c[0][2]).toBeCloseTo(1);
    const d = cells("strip", 3, 2 / 3, 3);
    expect(d).toHaveLength(6);
    expect(d[0][1]).toBeCloseTo(d[3][1]);
    expect(d[3][0]).toBeGreaterThan(0.5);
    expect(cells("polaroid", 1, 0.82, 10)).toEqual([POLAROID_CELL]);
    expect(columns("strip", 1 / 3)).toBe(1);
    expect(columns("strip", 2 / 3)).toBe(2);
    expect(columns("grid", 2 / 3)).toBe(1);
  });

  it("encre lisible sur le fond", () => {
    expect(inkFor("#ffffff")).toBe("#1a1a1a");
    expect(inkFor("#f6f3ea")).toBe("#1a1a1a");
    expect(inkFor("#101010")).toBe("#ffffff");
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
