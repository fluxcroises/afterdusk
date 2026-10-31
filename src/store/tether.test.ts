import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: vi.fn(async () => []),
}));

const { useLibrary } = await import("./library");
const { useNavigation } = await import("./navigation");
const { useTether, modelOf } = await import("./tether");

const photo = (id: number, companionOf: number | null = null) => ({
  id,
  albumId: 7,
  fileName: `DSCF${id}.RAF`,
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
  companionOf,
  copyOf: null,
});

const session = {
  camera: "FUJIFILM X-H2",
  active: true,
  connected: true,
  libraryId: 1,
  albumName: "X-H2 · 31/10/2026",
  folderId: 3,
  received: 1,
  remaining: 37,
  leftOnCard: 0,
  failed: [],
};

describe("photo reçue du boîtier", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    useNavigation.setState({ screen: "cull" });
    useLibrary.setState({
      libraryId: 1,
      albumId: 7,
      albums: [{ id: 7, name: session.albumName } as never],
      media: [photo(1), photo(2), photo(3, 2)],
      selectedId: 1,
      view: "grid",
      zoom: null,
    });
    useTether.setState({ showFor: 5 });
    useTether.getState().onStatus(session);
  });

  afterEach(() => {
    useTether.getState().onStatus({ ...session, active: false });
    vi.useRealTimers();
  });

  it("s'affiche en grand puis revient à la grille", async () => {
    await useTether.getState().imported({ folderId: 3, libraryId: 1, mediaIds: [2, 3] });
    // RAW + JPEG : la pile s'affiche par son fichier du dessus.
    expect(useLibrary.getState().selectedId).toBe(2);
    expect(useLibrary.getState().view).toBe("loupe");
    vi.advanceTimersByTime(4900);
    expect(useLibrary.getState().view).toBe("loupe");
    vi.advanceTimersByTime(200);
    expect(useLibrary.getState().view).toBe("grid");
  });

  it("reste ouverte quand la souris bouge", async () => {
    await useTether.getState().imported({ folderId: 3, libraryId: 1, mediaIds: [2] });
    vi.advanceTimersByTime(1000);
    window.dispatchEvent(new Event("pointermove"));
    vi.advanceTimersByTime(10_000);
    expect(useLibrary.getState().view).toBe("loupe");
  });

  it("la suivante remplace la précédente et relance le délai", async () => {
    await useTether.getState().imported({ folderId: 3, libraryId: 1, mediaIds: [1] });
    vi.advanceTimersByTime(4000);
    await useTether.getState().imported({ folderId: 3, libraryId: 1, mediaIds: [2] });
    vi.advanceTimersByTime(4000);
    expect(useLibrary.getState().selectedId).toBe(2);
    expect(useLibrary.getState().view).toBe("loupe");
  });

  it("ignore les autres dossiers, les autres écrans et le réglage « Non »", async () => {
    await useTether.getState().imported({ folderId: 9, libraryId: 1, mediaIds: [2] });
    expect(useLibrary.getState().view).toBe("grid");
    useNavigation.setState({ screen: "develop" });
    await useTether.getState().imported({ folderId: 3, libraryId: 1, mediaIds: [2] });
    expect(useLibrary.getState().view).toBe("grid");
    useNavigation.setState({ screen: "cull" });
    useTether.setState({ showFor: -1 });
    await useTether.getState().imported({ folderId: 3, libraryId: 1, mediaIds: [2] });
    expect(useLibrary.getState().view).toBe("grid");
  });

  it("« jusqu'à la suivante » ne ferme pas", async () => {
    useTether.setState({ showFor: 0 });
    await useTether.getState().imported({ folderId: 3, libraryId: 1, mediaIds: [2] });
    vi.advanceTimersByTime(60_000);
    expect(useLibrary.getState().view).toBe("loupe");
  });

  it("nom du modèle", () => {
    expect(modelOf("FUJIFILM X-H2")).toBe("X-H2");
    expect(modelOf("X-H2")).toBe("X-H2");
  });
});
