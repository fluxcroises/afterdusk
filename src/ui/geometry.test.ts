import { describe, expect, it } from "vitest";
import type { Geometry } from "../store/develop";
import { outputSize, outputToSource, sourceToOutput } from "./geometry";

const cases: Geometry[] = [
  {},
  { quarter: 1 },
  { quarter: 2, flipH: true },
  { quarter: 3, flipV: true, angle: -7.5 },
  { angle: 12, crop: { left: 0.1, top: 0.2, right: 0.8, bottom: 0.9 } },
  {
    quarter: 1,
    flipH: true,
    flipV: true,
    angle: 3,
    crop: { left: 0.3, top: 0, right: 1, bottom: 0.6 },
  },
];

describe("repères de l'image affichée et de la source", () => {
  it("aller et retour pour toute géométrie", () => {
    for (const g of cases) {
      for (const [fx, fy] of [
        [0.5, 0.5],
        [0.1, 0.8],
        [0.9, 0.05],
      ] as const) {
        const [sx, sy] = outputToSource(fx, fy, g, 600, 400);
        const [bx, by] = sourceToOutput(sx, sy, g, 600, 400);
        expect(bx).toBeCloseTo(fx, 9);
        expect(by).toBeCloseTo(fy, 9);
      }
    }
  });

  it("comme le cœur : quart de tour horaire, miroir, recadrage", () => {
    // Quart de tour horaire : le coin haut gauche affiché est le bas gauche de la source.
    const [x, y] = outputToSource(0, 0, { quarter: 1 }, 600, 400);
    expect(x).toBeCloseTo(0, 9);
    expect(y).toBeCloseTo(1, 9);
    const [mx] = outputToSource(0.25, 0.5, { flipH: true }, 600, 400);
    expect(mx).toBeCloseTo(0.75, 9);
    const crop = { crop: { left: 0.5, top: 0.5, right: 1, bottom: 1 } };
    expect(outputToSource(0, 0, crop, 600, 400)).toEqual([0.5, 0.5]);
    expect(outputSize({ quarter: 1, ...crop }, 600, 400)).toEqual([200, 300]);
  });
});
