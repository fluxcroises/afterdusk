import { describe, expect, it } from "vitest";
import rust from "../../src-tauri/src/develop/settings.rs?raw";
import { LOCAL_KEYS, MASKS_MAX, type Stroke } from "./develop";

// Le cœur (Rust) fait foi : mêmes réglages de masque et même limite des deux côtés.

const camel = (snake: string) => snake.replace(/_([a-z])/g, (_, c: string) => c.toUpperCase());

describe("masques : interface et cœur alignés", () => {
  it("mêmes réglages locaux", () => {
    const body = /pub struct LocalAdjust \{([\s\S]*?)\n\}/.exec(rust)?.[1] ?? "";
    const fields = [...body.matchAll(/pub (\w+): f64/g)].map((m) => camel(m[1] ?? ""));
    expect(fields).toEqual([...LOCAL_KEYS]);
  });

  it("mêmes champs d'un tracé de pinceau", () => {
    const body = /pub struct Stroke \{([\s\S]*?)\n\}/.exec(rust)?.[1] ?? "";
    const fields = [...body.matchAll(/pub (\w+):/g)].map((m) => camel(m[1] ?? ""));
    // Tous les champs du type de l'interface, ni plus ni moins (vérifié par tsc).
    const sample: Required<Stroke> = { points: [], radius: 0, feather: 0, flow: 0, erase: false };
    expect(fields).toEqual(Object.keys(sample));
    expect(rust).toContain("Brush,");
  });

  it("même nombre de masques au plus", () => {
    expect(rust).toContain(`pub const MASKS_MAX: usize = ${MASKS_MAX};`);
  });
});
