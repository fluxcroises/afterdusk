import { afterEach, describe, expect, it, vi } from "vitest";
import { hideSplash } from "./splash";

describe("écran de démarrage", () => {
  afterEach(() => {
    vi.useRealTimers();
    document.body.innerHTML = "";
  });

  it("disparaît en fondu puis est retiré du document", () => {
    vi.useFakeTimers();
    document.body.innerHTML = '<div id="splash"></div>';
    hideSplash();
    vi.advanceTimersByTime(2000);
    expect(document.getElementById("splash")?.classList.contains("splash-done")).toBe(true);
    vi.advanceTimersByTime(400);
    expect(document.getElementById("splash")).toBeNull();
  });
});
