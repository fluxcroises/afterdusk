import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn<(cmd: string, args?: Record<string, unknown>) => Promise<unknown>>();
vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: (cmd: string, args?: Record<string, unknown>) => invoke(cmd, args),
}));

const { useLibrary, daysLeft } = await import("./library");
const { useNavigation } = await import("./navigation");
const { handleShortcut } = await import("../desktop");

const photo = (id: number, flag = 0) => ({
  id,
  albumId: 1,
  fileName: `IMG_${id}.CR3`,
  capturedAt: null,
  rating: 0,
  flag,
  color: null,
  width: null,
  height: null,
  orientation: 1,
  hasPreview: false,
  trashedAt: null,
  stackSize: 1,
  companionOf: null,
});

/** Le cœur simulé : une bibliothèque de trois photos et sa corbeille. */
function fakeCore() {
  let normal = [photo(1), photo(2, -1), photo(3, -1)];
  let trashed: ReturnType<typeof photo>[] = [];
  invoke.mockReset().mockImplementation((cmd, args) => {
    const ids = (args?.ids as number[] | undefined) ?? [];
    switch (cmd) {
      case "media_trash":
        trashed = [...trashed, ...normal.filter((m) => ids.includes(m.id))];
        normal = normal.filter((m) => !ids.includes(m.id));
        return Promise.resolve({ done: ids.length, failed: 0 });
      case "media_restore":
        normal = [...normal, ...trashed.filter((m) => ids.includes(m.id))].sort(
          (a, b) => a.id - b.id,
        );
        trashed = trashed.filter((m) => !ids.includes(m.id));
        return Promise.resolve({ done: ids.length, failed: 0 });
      case "media_list":
        return Promise.resolve(args?.trashed ? trashed : normal);
      case "albums_list":
        return Promise.resolve([]);
      case "trash_summary":
        return Promise.resolve({ count: trashed.length, retentionDays: 30 });
      default:
        return Promise.resolve(null);
    }
  });
}

const key = (init: KeyboardEventInit) => new KeyboardEvent("keydown", init);

describe("corbeille", () => {
  beforeEach(() => {
    fakeCore();
    useNavigation.setState({ screen: "library" });
    useLibrary.setState({
      libraryId: 1,
      albumId: null,
      trash: false,
      media: [photo(1), photo(2, -1), photo(3, -1)],
      selectedId: 2,
      selectedIds: [2],
      view: "grid",
      notice: null,
      undoIds: null,
      error: null,
    });
  });

  it("envoie la sélection, puis Annuler la restaure", async () => {
    await useLibrary.getState().moveToTrash();
    expect(invoke).toHaveBeenCalledWith("media_trash", { ids: [2], wholePiles: true });
    let s = useLibrary.getState();
    expect(s.media.map((m) => m.id)).toEqual([1, 3]);
    expect(s.trashCount).toBe(1);
    // La voisine prend la place de la photo jetée.
    expect(s.selectedId).toBe(3);
    expect(s.notice).toEqual({ kind: "trashed", count: 1 });

    // L'annonce s'est effacée : Ctrl+Z annule encore.
    useLibrary.setState({ notice: null });
    await useLibrary.getState().undoTrash();
    expect(invoke).toHaveBeenCalledWith("media_restore", { ids: [2] });
    s = useLibrary.getState();
    expect(s.media.map((m) => m.id)).toEqual([1, 2, 3]);
    expect(s.trashCount).toBe(0);
    expect(s.notice?.kind).toBe("restored");
    expect(s.undoIds).toBeNull();
  });

  it("les rejetées de la vue", () => {
    expect(useLibrary.getState().rejectedIds()).toEqual([2, 3]);
  });

  it("dans la corbeille : pas de tri au clavier", async () => {
    await useLibrary.getState().moveToTrash([2, 3]);
    await useLibrary.getState().showTrash();
    const s = useLibrary.getState();
    expect(s.trash).toBe(true);
    expect(s.media.map((m) => m.id)).toEqual([2, 3]);
    invoke.mockClear();
    handleShortcut(key({ code: "Digit5", key: "5" }));
    expect(invoke).not.toHaveBeenCalledWith("media_set_rating", expect.anything());
    // Pas de nouvel envoi dans la corbeille depuis la corbeille.
    await useLibrary.getState().moveToTrash();
    expect(invoke).not.toHaveBeenCalledWith("media_trash", expect.anything());
  });

  it("jours restants avant la suppression définitive", () => {
    const now = Date.parse("2026-10-19T12:00:00Z");
    expect(daysLeft("2026-10-19T11:00:00Z", 30, now)).toBe(30);
    expect(daysLeft("2026-10-09T12:00:00Z", 30, now)).toBe(20);
    expect(daysLeft("2026-09-01T00:00:00Z", 30, now)).toBe(0);
    expect(daysLeft(null, 30, now)).toBe(30);
  });
});
