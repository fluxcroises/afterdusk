import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn<(cmd: string, args?: Record<string, unknown>) => Promise<unknown>>();
vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: (cmd: string, args?: Record<string, unknown>) => invoke(cmd, args),
}));

const { useLibrary } = await import("./library");
const { useBursts } = await import("./bursts");
const { useNavigation } = await import("./navigation");
const { handleShortcut } = await import("../desktop");

const photo = (id: number, burstId: number | null = null, burstSize = 1) => ({
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
  burstId,
  burstSize,
});

// Rafale 9 : la photo 2 au-dessus, 1 et 3 dessous ; la 4 seule.
const collapsed = [photo(2, 9, 3), photo(4)];
const opened = [photo(2, 9, 3), photo(1, 9, 3), photo(3, 9, 3), photo(4)];

describe("rafales", () => {
  beforeEach(() => {
    invoke.mockReset().mockImplementation((cmd, args) => {
      if (cmd === "media_list") {
        const expanded = (args?.expanded as number[] | undefined) ?? [];
        return Promise.resolve(expanded.includes(2) ? opened : collapsed);
      }
      if (cmd === "bursts_detect") return Promise.resolve({ bursts: 1, photos: 3 });
      return Promise.resolve(null);
    });
    useNavigation.setState({ screen: "cull" });
    useBursts.setState({ running: false, lastReport: null });
    useLibrary.setState({
      libraryId: 1,
      albumId: null,
      collectionId: null,
      trash: false,
      media: collapsed,
      selectedId: 2,
      selectedIds: [2],
      view: "grid",
      expanded: [],
      filter: {},
      error: null,
    });
  });

  it("S déplie la rafale depuis le dessus, et la replie depuis une photo du dessous", async () => {
    handleShortcut(new KeyboardEvent("keydown", { key: "s" }));
    await vi.waitFor(() => expect(useLibrary.getState().media).toHaveLength(4));
    expect(useLibrary.getState().expanded).toEqual([2]);
    await useLibrary.getState().select(3);
    await useLibrary.getState().toggleStack();
    expect(useLibrary.getState().expanded).toEqual([]);
    expect(useLibrary.getState().media).toHaveLength(2);
    // La rafale reste sélectionnée par sa photo du dessus.
    expect(useLibrary.getState().selectedId).toBe(2);
  });

  it("une photo du dessous passe au-dessus, la rafale se replie sur elle", async () => {
    useLibrary.setState({ media: opened, expanded: [2], selectedId: 3, selectedIds: [3] });
    await useLibrary.getState().setStackTop(3);
    expect(invoke).toHaveBeenCalledWith("pile_set_top", { id: 3 });
    expect(useLibrary.getState().expanded).toEqual([]);
  });

  it("détection, empilage (photo active au-dessus) et retrait", async () => {
    await useBursts.getState().detect(5);
    expect(invoke).toHaveBeenCalledWith("bursts_detect", { libraryId: 1, albumId: 5 });
    expect(useBursts.getState().lastReport).toEqual({ bursts: 1, photos: 3 });

    useLibrary.setState({ media: opened, selectedId: 3, selectedIds: [1, 3, 4] });
    await useBursts.getState().stackSelection();
    expect(invoke).toHaveBeenCalledWith("bursts_stack", { libraryId: 1, ids: [3, 1, 4] });

    useLibrary.setState({ selectedId: 1, selectedIds: [1] });
    await useBursts.getState().unstackSelection();
    expect(invoke).toHaveBeenCalledWith("bursts_unstack", { ids: [1] });

    // Une seule photo : rien à empiler.
    invoke.mockClear();
    await useBursts.getState().stackSelection();
    expect(invoke.mock.calls.some(([cmd]) => cmd === "bursts_stack")).toBe(false);
  });

  it("la préférence de regroupement après l'import se garde", () => {
    useBursts.getState().setAuto(false);
    expect(window.localStorage.getItem("afterdusk.bursts.auto")).toBe("false");
    useBursts.getState().setAuto(true);
    expect(useBursts.getState().auto).toBe(true);
  });
});
