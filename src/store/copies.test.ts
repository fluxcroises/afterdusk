import { describe, expect, it } from "vitest";
import { stackTop } from "./library";

describe("copies virtuelles dans les grilles", () => {
  it("une copie compte pour sa photo (mêmes marques)", () => {
    expect(stackTop({ id: -4, companionOf: null, copyOf: 12 })).toBe(12);
    expect(stackTop({ id: 13, companionOf: 12 })).toBe(12);
    expect(stackTop({ id: 12, companionOf: null, copyOf: null })).toBe(12);
  });
});
