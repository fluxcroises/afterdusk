import { describe, expect, it } from "vitest";
import { controlKey, isTyping } from "./typing";

function input(type: string) {
  const el = document.createElement("input");
  el.type = type;
  return el;
}

function key(target: EventTarget, k: string) {
  const e = new KeyboardEvent("keydown", { key: k });
  Object.defineProperty(e, "target", { value: target });
  return e;
}

describe("raccourcis et champs", () => {
  it("une case cochée ou un curseur n'est pas une saisie : Ctrl+Z y reste actif", () => {
    expect(isTyping(input("checkbox"))).toBe(false);
    expect(isTyping(input("range"))).toBe(false);
    expect(isTyping(input("text"))).toBe(true);
    expect(isTyping(document.createElement("textarea"))).toBe(true);
    expect(isTyping(document.createElement("select"))).toBe(true);
  });

  it("Espace et Entrée restent à la case ou au bouton", () => {
    expect(controlKey(key(input("checkbox"), " "))).toBe(true);
    expect(controlKey(key(document.createElement("button"), "Enter"))).toBe(true);
    expect(controlKey(key(input("checkbox"), "z"))).toBe(false);
    expect(controlKey(key(document.body, " "))).toBe(false);
  });
});
