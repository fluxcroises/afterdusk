import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn<(cmd: string, args?: Record<string, unknown>) => Promise<unknown>>();
vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: (cmd: string, args?: Record<string, unknown>) => invoke(cmd, args),
}));

const { useLibrary, isFiltering, visibleAlbums, fold } = await import("./library");
type Album = Parameters<typeof visibleAlbums>[0][number];

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
  stackSize: 1,
  companionOf: null,
});

const album = (id: number, name: string, startDate: string, place: string | null): Album => ({
  id,
  name,
  folder: name,
  startDate,
  endDate: startDate,
  count: 1,
  place,
  coverMediaId: null,
  coverChosen: false,
});

describe("filtres de la grille", () => {
  beforeEach(() => {
    invoke.mockReset().mockImplementation((cmd, args) => {
      if (cmd !== "media_list") return Promise.resolve(null);
      const filter = (args?.filter ?? {}) as { flags?: number[] };
      // Le cœur simulé : la photo 2 seule est retenue.
      return Promise.resolve(filter.flags?.includes(1) ? [photo(2)] : [photo(1), photo(2)]);
    });
    useLibrary.setState({
      libraryId: 1,
      albumId: null,
      trash: false,
      media: [photo(1), photo(2)],
      selectedId: 1,
      selectedIds: [1],
      filter: {},
      sort: "captureTime",
    });
  });

  it("transmet filtre et tri au cœur, et garde une sélection visible", async () => {
    await useLibrary.getState().setFilter({ flags: [1], query: "plage" });
    expect(invoke).toHaveBeenCalledWith("media_list", {
      libraryId: 1,
      albumId: null,
      trashed: false,
      filter: { flags: [1], query: "plage" },
      sort: "captureTime",
      expanded: [],
    });
    const s = useLibrary.getState();
    expect(s.media.map((m) => m.id)).toEqual([2]);
    // La photo active a disparu de la grille : la première visible prend sa place.
    expect(s.selectedId).toBe(2);
  });

  it("un champ vidé disparaît du filtre", async () => {
    await useLibrary.getState().setFilter({ flags: [1], query: "plage", cameraId: 3 });
    await useLibrary.getState().setFilter({ flags: [], query: "", cameraId: undefined });
    expect(useLibrary.getState().filter).toEqual({});
    expect(isFiltering(useLibrary.getState().filter)).toBe(false);
    expect(isFiltering({ query: "  " })).toBe(false);
    expect(isFiltering({ rating: { op: "exactly", value: 0 } })).toBe(true);
  });

  it("le tri recharge la grille", async () => {
    await useLibrary.getState().setSort("rating");
    expect(invoke).toHaveBeenLastCalledWith(
      "media_list",
      expect.objectContaining({ sort: "rating" }),
    );
  });
});

describe("vue Albums", () => {
  const albums = [
    album(1, "Été à Cabourg", "2026-07-14", "Cabourg"),
    album(2, "Lisbonne", "2025-04-02", null),
    album(3, "Anniversaire", "2026-01-20", "Caen"),
  ];

  it("recherche sans accents ni casse, sur le nom et le lieu", () => {
    expect(fold("Été À")).toBe("ete a");
    expect(visibleAlbums(albums, "ete", "date").map((a) => a.id)).toEqual([1]);
    expect(visibleAlbums(albums, "CAEN", "date").map((a) => a.id)).toEqual([3]);
    expect(visibleAlbums(albums, "", "date")).toHaveLength(3);
  });

  it("tri par date, nom ou lieu (sans lieu en dernier)", () => {
    expect(visibleAlbums(albums, "", "date").map((a) => a.id)).toEqual([1, 3, 2]);
    expect(visibleAlbums(albums, "", "name").map((a) => a.id)).toEqual([3, 1, 2]);
    expect(visibleAlbums(albums, "", "place").map((a) => a.id)).toEqual([1, 3, 2]);
  });
});
