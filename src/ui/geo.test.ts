import { describe, expect, it } from "vitest";
import { areaOf, inArea, toMicro, wrapLongitude } from "./geo";

describe("zones de la carte", () => {
  it("zone ordinaire, coins dans n'importe quel ordre", () => {
    const area = areaOf([2.5, 49], [-1, 43]);
    expect(area).toEqual([-1, 43, 2.5, 49]);
    expect(inArea(2.35, 48.85, area)).toBe(true);
    expect(inArea(5.37, 43.3, area)).toBe(false);
    expect(toMicro(area)).toEqual([-1_000_000, 43_000_000, 2_500_000, 49_000_000]);
  });

  it("zone à cheval sur 180°, carte déroulée au-delà d'un tour", () => {
    // Tracée de 170° E à 190° E (soit -170°) sur la carte déroulée.
    const area = areaOf([170, -20], [190, -10]);
    expect(area).toEqual([170, -20, -170, -10]);
    expect(inArea(179, -16, area)).toBe(true);
    expect(inArea(-175, -16, area)).toBe(true);
    expect(inArea(0, -16, area)).toBe(false);
    expect(wrapLongitude(540)).toBe(-180);
    expect(wrapLongitude(-190)).toBe(170);
  });
});
