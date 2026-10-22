import { describe, expect, it } from "vitest";
import { ASPECTS, aspectRatio, orientedAspect, swapAspect } from "./develop";

const crop = (left: number, top: number, right: number, bottom: number) => ({
  left,
  top,
  right,
  bottom,
});

describe("ratios de recadrage", () => {
  it("quinze ratios comme dans Photos, chacun dans son sens", () => {
    expect(ASPECTS).toHaveLength(15);
    expect(aspectRatio("4:5", 1.5)).toBeCloseTo(0.8);
    expect(aspectRatio("5:4", 1.5)).toBeCloseTo(1.25);
    expect(aspectRatio("original", 1.5)).toBe(1.5);
    expect(aspectRatio("free", 1.5)).toBeNull();
  });

  it("portrait ou paysage : le ratio inverse, sauf libre, original et carré", () => {
    expect(swapAspect("9:16")).toBe("16:9");
    expect(swapAspect("7:5")).toBe("5:7");
    for (const a of ["free", "original", "1:1"]) expect(swapAspect(a)).toBe(a);
  });

  it("un ratio d'avant les ratios orientés suit le sens du cadre", () => {
    // Photo 3:2 en paysage : un cadre de 0,5 × 1 (en fractions) est en portrait.
    const tall = crop(0.25, 0, 0.75, 1);
    const wide = crop(0, 0, 1, 1);
    expect(orientedAspect("3:2", tall, 1.5)).toBe("2:3");
    expect(orientedAspect("3:2", wide, 1.5)).toBe("3:2");
    expect(orientedAspect("2:3", tall, 1.5)).toBe("2:3");
    expect(orientedAspect("1:1", tall, 1.5)).toBe("1:1");
    expect(orientedAspect(undefined, tall, 1.5)).toBe("free");
  });
});
