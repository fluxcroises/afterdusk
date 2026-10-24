import { describe, expect, it } from "vitest";
import type { Mask } from "../store/develop";
import { maskViewPixels, maskWeight, movedShape, normalizeAngle } from "./masks";

// Mêmes cas que les tests de `develop::masks` dans le cœur (image de 400 × 200).
const ciel: Mask = { shape: { kind: "linear", from: [0.5, 0], to: [0.5, 0.5] } };
const rond = (feather: number): Mask => ({
  shape: { kind: "radial", center: [0.5, 0.5], radius: [0.25, 0.5], feather },
});
const at = (m: Mask, x: number, y: number) => maskWeight(m, x + 0.5, y + 0.5, 400, 200);

describe("poids des masques, comme le cœur", () => {
  it("dégradé linéaire : plein, moitié, nul", () => {
    expect(at(ciel, 200, 0)).toBeGreaterThan(0.99);
    expect(Math.abs(at(ciel, 200, 50) - 0.5)).toBeLessThan(0.02);
    expect(at(ciel, 200, 100)).toBe(0);
    expect(at(ciel, 0, 30)).toBe(at(ciel, 399, 30));
  });

  it("dégradé radial en pixels de la source : un cercle sur une image 2:1", () => {
    expect(at(rond(0), 290, 100)).toBe(1);
    expect(at(rond(0), 200, 190)).toBe(1);
    expect(at(rond(0), 310, 100)).toBe(0);
    expect(at(rond(0), 275, 175)).toBe(0);
  });

  it("adoucissement, inversion et rotation", () => {
    const mi = at(rond(100), 250, 100);
    expect(mi).toBeGreaterThan(0.3);
    expect(mi).toBeLessThan(0.7);
    expect(at({ ...rond(100), invert: true }, 250, 100)).toBeCloseTo(1 - mi, 6);
    const fine: Mask = {
      shape: { kind: "radial", center: [0.5, 0.5], radius: [0.2, 0.05], angle: 90, feather: 0 },
    };
    expect(at(fine, 200, 160)).toBe(1);
    expect(at(fine, 260, 100)).toBe(0);
  });

  it("calque rouge : opacité selon le poids", () => {
    const px = maskViewPixels(2, 1, (fx) => [fx, 0], { shape: ciel.shape, invert: true }, 400, 200);
    expect([...px.slice(0, 4)]).toEqual([230, 40, 40, 0]);
    expect(px[7]).toBe(0);
  });

  it("angle ramené et forme déplacée", () => {
    expect(normalizeAngle(190)).toBe(-170);
    expect(normalizeAngle(-180)).toBe(-180);
    expect(normalizeAngle(180)).toBe(-180);
    expect(movedShape(ciel.shape, 0.1, 0.2)).toEqual({
      kind: "linear",
      from: [0.6, 0.2],
      to: [0.6, 0.7],
    });
  });
});
