import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn<(...args: unknown[]) => Promise<number>>(async () => 1);
vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: (...args: unknown[]) => invoke(...args),
}));

const { compareIds, useLibrary } = await import("./library");
const { useNavigation } = await import("./navigation");
const { handleShortcut } = await import("../desktop");
const { Compare } = await import("../components/Compare");
const { default: i18n } = await import("../i18n");

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
  hasPreview: false,
  trashedAt: null,
});

const press = (key: string, code = "") =>
  handleShortcut(new KeyboardEvent("keydown", { key, code }));

describe("sélection multiple et comparaison", () => {
  beforeEach(async () => {
    await i18n.changeLanguage("fr");
    invoke.mockClear();
    useNavigation.setState({ screen: "cull" });
    useLibrary.setState({
      media: [1, 2, 3, 4, 5, 6].map(photo),
      selectedId: 1,
      selectedIds: [1],
      view: "grid",
      zoom: null,
      autoAdvance: false,
    });
  });

  it("Ctrl+clic ajoute et retire, Maj+clic prend la plage", async () => {
    const lib = useLibrary.getState();
    await lib.select(3, "toggle");
    expect(useLibrary.getState().selectedIds).toEqual([1, 3]);
    expect(useLibrary.getState().selectedId).toBe(3);
    await useLibrary.getState().select(3, "toggle");
    expect(useLibrary.getState().selectedIds).toEqual([1]);
    expect(useLibrary.getState().selectedId).toBe(1);
    await useLibrary.getState().select(4, "range");
    expect(useLibrary.getState().selectedIds).toEqual([1, 2, 3, 4]);
  });

  it("dans la grille, une note s'applique à toute la sélection", async () => {
    await useLibrary.getState().select(2, "toggle");
    await useLibrary.getState().setRating(3);
    const ratings = useLibrary.getState().media.map((m) => m.rating);
    expect(ratings).toEqual([3, 3, 0, 0, 0, 0]);
    expect(invoke).toHaveBeenCalledWith("media_set_rating", { ids: [1, 2], rating: 3 });
  });

  it("compare la sélection (4 au plus, dans l'ordre de la grille) ou la photo et la suivante", () => {
    expect(compareIds({ media: [1, 2, 3].map(photo), selectedId: 2, selectedIds: [2] })).toEqual([
      2, 3,
    ]);
    expect(compareIds({ media: [1, 2, 3].map(photo), selectedId: 3, selectedIds: [3] })).toEqual([
      3, 2,
    ]);
    expect(
      compareIds({
        media: [1, 2, 3, 4, 5, 6].map(photo),
        selectedId: 6,
        selectedIds: [6, 2, 5, 1, 3],
      }),
    ).toEqual([1, 2, 3, 5]);
  });

  it("C ouvre la comparaison ; les flèches changent le panneau actif ; la note va au panneau actif", async () => {
    await useLibrary.getState().select(4, "range");
    press("c", "KeyC");
    expect(useLibrary.getState().view).toBe("compare");
    // Après Maj+clic, la 4 est active ; la flèche droite revient à la 1.
    press("ArrowRight");
    expect(useLibrary.getState().selectedId).toBe(1);
    press("ArrowRight");
    expect(useLibrary.getState().selectedId).toBe(2);
    press("ArrowLeft");
    press("ArrowLeft");
    expect(useLibrary.getState().selectedId).toBe(4);
    press("5", "Digit5");
    expect(useLibrary.getState().media.map((m) => m.rating)).toEqual([0, 0, 0, 5, 0, 0]);
    press("c", "KeyC");
    expect(useLibrary.getState().view).toBe("grid");
  });

  it("affiche un panneau par photo et active celui qu'on clique", async () => {
    await useLibrary.getState().select(3, "range");
    useLibrary.getState().setView("compare");
    const { container } = render(<Compare />);
    expect(container.querySelector(".compare")).toHaveAttribute("data-count", "3");
    expect(screen.getAllByRole("figure")).toHaveLength(3);
    const second = screen.getAllByRole("figure")[1];
    if (second) fireEvent.pointerDown(second.querySelector(".loupe") as Element);
    expect(useLibrary.getState().selectedId).toBe(2);
  });
});
