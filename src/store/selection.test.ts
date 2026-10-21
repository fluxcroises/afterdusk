import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn<(cmd: string, args?: Record<string, unknown>) => Promise<unknown>>();
vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: (cmd: string, args?: Record<string, unknown>) => invoke(cmd, args),
}));

const { useLibrary } = await import("./library");
const { useImport } = await import("./import");
const { useNavigation } = await import("./navigation");
const { selectAll } = await import("../menu");

const photo = (id: number) => ({
  id,
  albumId: 1,
  fileName: `DSC_000${id}.NEF`,
  capturedAt: null,
  rating: 0,
  flag: 0,
  color: null,
  width: null,
  height: null,
  orientation: 1,
  hasPreview: false,
  trashedAt: null,
  stackSize: 1,
  companionOf: null,
  copyOf: null,
  version: null,
});

describe("Édition > Tout sélectionner", () => {
  beforeEach(() => {
    invoke.mockReset().mockResolvedValue(null);
    useLibrary.setState({
      media: [photo(1), photo(2), photo(3)],
      selectedId: 2,
      selectedIds: [2],
      showAlbums: false,
    });
  });
  afterEach(() => document.body.replaceChildren());

  it("sélectionne toutes les photos affichées et garde la photo active", () => {
    for (const screen of ["cull", "library", "develop", "deliver"] as const) {
      useLibrary.setState({ selectedIds: [2] });
      useNavigation.setState({ screen });
      selectAll();
      expect(useLibrary.getState().selectedIds, screen).toEqual([1, 2, 3]);
      expect(useLibrary.getState().selectedId).toBe(2);
    }
  });

  it("dans un champ, sélectionne son texte et non les photos", () => {
    useNavigation.setState({ screen: "library" });
    const input = document.createElement("input");
    input.value = "Cabourg";
    document.body.append(input);
    input.focus();
    selectAll();
    expect([input.selectionStart, input.selectionEnd]).toEqual([0, 7]);
    expect(useLibrary.getState().selectedIds).toEqual([2]);
  });

  it("rien quand la vue montre les albums ; à l'import, toute la carte", () => {
    useNavigation.setState({ screen: "library" });
    useLibrary.setState({ showAlbums: true });
    selectAll();
    expect(useLibrary.getState().selectedIds).toEqual([2]);
    const spy = vi.spyOn(useImport.getState(), "selectAll");
    useNavigation.setState({ screen: "import" });
    selectAll();
    expect(spy).toHaveBeenCalledWith(true);
  });
});
