import { describe, expect, it } from "vitest";
import { curveAt } from "./curve";

describe("courbe de tonalité affichée", () => {
  it("identité sans points, passe par chaque point", () => {
    expect(curveAt([])(100)).toBe(100);
    const f = curveAt([
      [0, 0],
      [64, 40],
      [192, 215],
      [255, 255],
    ]);
    expect(f(64)).toBeCloseTo(40, 6);
    expect(f(192)).toBeCloseTo(215, 6);
  });

  it("monotone et à plat hors des extrémités, comme dans le cœur", () => {
    const f = curveAt([
      [0, 0],
      [100, 10],
      [110, 240],
      [255, 255],
    ]);
    let prev = -1;
    for (let x = 0; x <= 255; x++) {
      const y = f(x);
      expect(y).toBeGreaterThanOrEqual(prev - 1e-9);
      expect(y).toBeLessThanOrEqual(255);
      prev = y;
    }
    const fade = curveAt([
      [30, 20],
      [255, 230],
    ]);
    expect(fade(10)).toBeCloseTo(20, 6);
    expect(fade(255)).toBeCloseTo(230, 6);
  });
});
