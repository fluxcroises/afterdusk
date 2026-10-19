import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn<(cmd: string, args?: Record<string, unknown>) => Promise<unknown>>();
vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: (cmd: string, args?: Record<string, unknown>) => invoke(cmd, args),
}));

const { useLibrary } = await import("./library");
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
  width: null,
  height: null,
  orientation: 1,
  hasPreview: false,
  trashedAt: null,
});

const key = (init: KeyboardEventInit) => new KeyboardEvent("keydown", init);
const marks = () => useLibrary.getState().media.map((m) => [m.rating, m.flag, m.color]);

describe("notes, drapeaux et couleurs", () => {
  beforeEach(() => {
    invoke.mockReset().mockResolvedValue(1);
    useNavigation.setState({ screen: "cull" });
    useLibrary.setState({
      media: [photo(1), photo(2), photo(3)],
      selectedId: 1,
      autoAdvance: false,
      error: null,
    });
  });

  it("au clavier : chiffres, P, X, U, et 6 à 9 pour les couleurs", async () => {
    handleShortcut(key({ code: "Digit4", key: "4" }));
    handleShortcut(key({ code: "KeyP", key: "p" }));
    handleShortcut(key({ code: "Digit6", key: "6" }));
    expect(marks()[0]).toEqual([4, 1, "red"]);

    handleShortcut(key({ code: "Numpad0", key: "0" }));
    handleShortcut(key({ code: "KeyX", key: "x" }));
    handleShortcut(key({ code: "Digit6", key: "6" })); // même couleur : retirée
    expect(marks()[0]).toEqual([0, -1, null]);

    handleShortcut(key({ code: "KeyU", key: "u" }));
    expect(marks()[0]).toEqual([0, 0, null]);

    await Promise.resolve();
    expect(invoke).toHaveBeenCalledWith("media_set_rating", { ids: [1], rating: 4 });
    expect(invoke).toHaveBeenCalledWith("media_set_flag", { ids: [1], flag: 1 });
    expect(invoke).toHaveBeenCalledWith("media_set_color", { ids: [1], color: "red" });
    expect(invoke).toHaveBeenCalledWith("media_set_color", { ids: [1], color: null });
  });

  it("clavier AZERTY : la touche 3 avec Maj donne aussi 3 étoiles", () => {
    handleShortcut(key({ code: "Digit3", key: '"', shiftKey: true }));
    expect(marks()[0]?.[0]).toBe(3);
  });

  it("inactif hors de Trier et Bibliothèque", () => {
    useNavigation.setState({ screen: "import" });
    expect(handleShortcut(key({ code: "Digit5", key: "5" }))).toBe(false);
    expect(marks()[0]?.[0]).toBe(0);
  });

  it("rétablit la valeur si le cœur refuse", async () => {
    invoke.mockRejectedValueOnce({ code: "internal", detail: "base verrouillée" });
    await useLibrary.getState().setRating(5);
    expect(marks()[0]?.[0]).toBe(0);
    expect(useLibrary.getState().error?.code).toBe("internal");
  });

  it("avance automatique après une note ou un drapeau, pas après une couleur", async () => {
    useLibrary.getState().setAutoAdvance(true);
    await useLibrary.getState().setRating(2);
    expect(useLibrary.getState().selectedId).toBe(2);
    await useLibrary.getState().toggleColor("green");
    expect(useLibrary.getState().selectedId).toBe(2);
    await useLibrary.getState().setFlag(1);
    expect(useLibrary.getState().selectedId).toBe(3);
    expect(marks()).toEqual([
      [2, 0, null],
      [0, 1, "green"],
      [0, 0, null],
    ]);
  });
});
