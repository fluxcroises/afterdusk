import { describe, expect, it } from "vitest";
import en from "./en.json";
import fr from "./fr.json";

function keys(obj: object, prefix = ""): string[] {
  return Object.entries(obj).flatMap(([k, v]) =>
    typeof v === "object" && v !== null ? keys(v, `${prefix}${k}.`) : [`${prefix}${k}`],
  );
}

describe("traductions", () => {
  it("fr.json et en.json ont exactement les mêmes clés", () => {
    expect(keys(fr).sort()).toEqual(keys(en).sort());
  });
});
