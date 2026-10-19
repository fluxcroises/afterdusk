import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn<(cmd: string, args?: Record<string, unknown>) => Promise<unknown>>();
vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: (cmd: string, args?: Record<string, unknown>) => invoke(cmd, args),
}));

const { useLibrary } = await import("./library");
const { useNavigation } = await import("./navigation");
const { handleShortcut } = await import("../desktop");

const photo = (id: number, fileName: string, stackSize = 1, companionOf: number | null = null) => ({
  id,
  albumId: 1,
  fileName,
  capturedAt: null,
  rating: 0,
  flag: 0,
  color: null,
  width: null,
  height: null,
  orientation: 1,
  hasPreview: false,
  trashedAt: null,
  stackSize,
  companionOf,
});

const raw = photo(1, "DSC_0001.NEF", 2);
const jpeg = photo(2, "DSC_0001.JPG", 1, 1);
const alone = photo(3, "DSC_0002.NEF");

const key = (init: KeyboardEventInit) => new KeyboardEvent("keydown", init);

describe("piles RAW + JPEG", () => {
  beforeEach(() => {
    invoke.mockReset().mockImplementation((cmd, args) => {
      if (cmd === "media_list") {
        const expanded = (args?.expanded as number[] | undefined) ?? [];
        return Promise.resolve(expanded.includes(1) ? [raw, jpeg, alone] : [raw, alone]);
      }
      if (cmd === "media_trash") return Promise.resolve({ done: 1, failed: 0 });
      return Promise.resolve(1);
    });
    useNavigation.setState({ screen: "cull" });
    useLibrary.setState({
      libraryId: 1,
      albumId: null,
      trash: false,
      media: [raw, alone],
      selectedId: 1,
      selectedIds: [1],
      view: "grid",
      expanded: [],
      filter: {},
      autoAdvance: false,
      error: null,
    });
  });

  it("S déplie puis replie la pile de la photo active", async () => {
    handleShortcut(key({ code: "KeyS", key: "s" }));
    await vi.waitFor(() => expect(useLibrary.getState().media).toHaveLength(3));
    expect(useLibrary.getState().expanded).toEqual([1]);
    expect(invoke).toHaveBeenCalledWith("media_list", expect.objectContaining({ expanded: [1] }));
    // Replier depuis le JPEG : la pile reste sélectionnée.
    await useLibrary.getState().select(2);
    await useLibrary.getState().toggleStack();
    const s = useLibrary.getState();
    expect(s.expanded).toEqual([]);
    expect(s.media.map((m) => m.id)).toEqual([1, 3]);
    expect(s.selectedId).toBe(1);
  });

  it("une note posée sur la pile vaut pour ses deux fichiers", async () => {
    await useLibrary.getState().toggleStack(1);
    await useLibrary.getState().select(1);
    await useLibrary.getState().setRating(4);
    const ratings = useLibrary.getState().media.map((m) => [m.id, m.rating]);
    expect(ratings).toEqual([
      [1, 4],
      [2, 4],
      [3, 0],
    ]);
  });

  it("Annuler après « Envoyer le RAW » restaure le RAW, pas la photo affichée", async () => {
    invoke.mockImplementation((cmd, args) => {
      if (cmd === "media_trash_format") return Promise.resolve({ done: 1, failed: 0, moved: [1] });
      if (cmd === "media_list") return Promise.resolve([jpeg, alone]);
      if (cmd === "media_restore") return Promise.resolve({ done: 1, failed: 0 });
      return Promise.resolve(args ? 1 : null);
    });
    await useLibrary.getState().select(2);
    await useLibrary.getState().trashFormat("raw");
    expect(invoke).toHaveBeenCalledWith("media_trash_format", { ids: [2], format: "raw" });
    await useLibrary.getState().undoTrash();
    expect(invoke).toHaveBeenCalledWith("media_restore", { ids: [1] });
  });

  it("Suppr : la pile entière si elle est repliée, le fichier choisi si elle est dépliée", async () => {
    await useLibrary.getState().moveToTrash([1]);
    expect(invoke).toHaveBeenCalledWith("media_trash", { ids: [1], wholePiles: true });
    await useLibrary.getState().toggleStack(1);
    await useLibrary.getState().moveToTrash([2]);
    expect(invoke).toHaveBeenCalledWith("media_trash", { ids: [2], wholePiles: false });
  });
});
