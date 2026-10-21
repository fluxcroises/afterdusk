import { describe, expect, it } from "vitest";
import { apertureBars, focalBars, monthBars } from "./gearStats";

describe("statistiques du matériel", () => {
  it("regroupe les focales par tranches quand il y en a trop", () => {
    const few = [
      { value: 24, count: 1 },
      { value: 50, count: 2 },
    ];
    expect(focalBars(few).map((b) => b.label)).toEqual(["24 mm", "50 mm"]);
    const many = Array.from({ length: 177 }, (_, i) => ({ value: 24 + i, count: 1 }));
    const bars = focalBars(many);
    expect(bars.length).toBeLessThanOrEqual(24);
    expect(bars[0].label).toBe("20–39 mm");
    expect(bars.reduce((n, b) => n + b.count, 0)).toBe(177);
  });

  it("garde les ouvertures les plus fréquentes, rangées par valeur", () => {
    const values = Array.from({ length: 20 }, (_, i) => ({ value: i + 1, count: i }));
    const bars = apertureBars(values);
    expect(bars).toHaveLength(16);
    expect(bars[0].label).toBe("f/5");
  });

  it("compte les mois vides, puis passe aux années", () => {
    const short = monthBars(
      [
        { month: "2025-11", count: 3 },
        { month: "2026-02", count: 1 },
      ],
      "fr",
    );
    expect(short.map((b) => b.count)).toEqual([3, 0, 0, 1]);
    const long = monthBars(
      [
        { month: "2020-01", count: 3 },
        { month: "2024-06", count: 1 },
      ],
      "fr",
    );
    expect(long.map((b) => b.label)).toEqual(["2020", "2021", "2022", "2023", "2024"]);
  });
});
