// @vitest-environment node
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { LOCAL_KEYS, MASKS_MAX } from "./develop";

// Le cœur (Rust) fait foi : mêmes réglages de masque et même limite des deux côtés.
const rust = readFileSync(join(__dirname, "../../src-tauri/src/develop/settings.rs"), "utf8");

const camel = (snake: string) => snake.replace(/_([a-z])/g, (_, c: string) => c.toUpperCase());

describe("masques : interface et cœur alignés", () => {
  it("mêmes réglages locaux", () => {
    const body = /pub struct LocalAdjust \{([\s\S]*?)\n\}/.exec(rust)?.[1] ?? "";
    const fields = [...body.matchAll(/pub (\w+): f64/g)].map((m) => camel(m[1] ?? ""));
    expect(fields).toEqual([...LOCAL_KEYS]);
  });

  it("même nombre de masques au plus", () => {
    expect(rust).toContain(`pub const MASKS_MAX: usize = ${MASKS_MAX};`);
  });
});
