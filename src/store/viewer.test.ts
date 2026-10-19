import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: vi.fn(async () => null),
}));

const { useLibrary } = await import("./library");
const { useLayout } = await import("./layout");
const { useNavigation } = await import("./navigation");
const { handleShortcut } = await import("../desktop");

const photo = (id: number) => ({
  id,
  albumId: 1,
  fileName: `IMG_${id}.CR3`,
  capturedAt: null,
  rating: 0,
  flag: 0,
  color: null,
  width: 6000,
  height: 4000,
  orientation: 1,
  hasPreview: true,
  trashedAt: null,
  stackSize: 1,
  companionOf: null,
});

const press = (key: string, code = "") =>
  handleShortcut(new KeyboardEvent("keydown", { key, code }));

describe("visionneuse au clavier", () => {
  beforeEach(() => {
    useNavigation.setState({ screen: "cull" });
    useLayout.setState({ immersive: false });
    useLibrary.setState({
      media: [photo(1), photo(2), photo(3)],
      selectedId: 1,
      view: "grid",
      zoom: null,
      navigatedAt: 0,
    });
  });

  it("Entrée ouvre la visionneuse, G revient à la grille", () => {
    press("Enter");
    expect(useLibrary.getState().view).toBe("loupe");
    press("g", "KeyG");
    expect(useLibrary.getState().view).toBe("grid");
  });

  it("Espace bascule entre ajusté et 100 % au centre", () => {
    press("Enter");
    press(" ");
    expect(useLibrary.getState().zoom).toEqual({ fx: 0.5, fy: 0.5 });
    press(" ");
    expect(useLibrary.getState().zoom).toBeNull();
  });

  it("changer de photo revient à l'ajusté et date la navigation", () => {
    press("Enter");
    press(" ");
    press("ArrowRight");
    const s = useLibrary.getState();
    expect(s.selectedId).toBe(2);
    expect(s.zoom).toBeNull();
    expect(s.navigatedAt).toBeGreaterThan(0);
  });

  it("F passe en plein écran ; Échap défait dans l'ordre : plein écran, zoom, visionneuse", () => {
    press("f", "KeyF");
    expect(useLayout.getState().immersive).toBe(true);
    expect(useLibrary.getState().view).toBe("loupe");
    press(" ");
    press("Escape");
    expect(useLayout.getState().immersive).toBe(false);
    expect(useLibrary.getState().zoom).not.toBeNull();
    press("Escape");
    expect(useLibrary.getState().zoom).toBeNull();
    press("Escape");
    expect(useLibrary.getState().view).toBe("grid");
    expect(press("Escape")).toBe(false);
  });
});
