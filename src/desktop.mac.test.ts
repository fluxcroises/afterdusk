import { describe, expect, it, vi } from "vitest";

vi.mock("./platform", async (original) => ({
  ...(await original<typeof import("./platform")>()),
  isMac: true,
  drawsOwnMenus: false,
}));

const { handleShortcut } = await import("./desktop");
const { useLayout } = await import("./store/layout");
const { useNavigation } = await import("./store/navigation");

describe("raccourcis sous macOS", () => {
  it("⌘F montre la barre de filtres et donne le focus à la recherche", async () => {
    useNavigation.setState({ screen: "cull" });
    useLayout.setState({ filterBar: false });
    const focus = vi.fn();
    window.addEventListener("afterdusk:focus-search", focus);
    const e = new KeyboardEvent("keydown", { key: "f", metaKey: true });
    expect(handleShortcut(e)).toBe(true);
    expect(useLayout.getState().filterBar).toBe(true);
    await new Promise((r) => setTimeout(r, 0));
    expect(focus).toHaveBeenCalled();
  });
});
