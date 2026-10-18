import { describe, expect, it } from "vitest";
import { greetingKey } from "./greeting";

describe("salutation", () => {
  it("Bonjour en journée, Bonsoir le soir et la nuit", () => {
    expect(greetingKey(new Date(2026, 9, 18, 9))).toBe("home.morning");
    expect(greetingKey(new Date(2026, 9, 18, 18))).toBe("home.evening");
    expect(greetingKey(new Date(2026, 9, 18, 2))).toBe("home.evening");
  });
});
