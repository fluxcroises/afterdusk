import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn<(cmd: string, args?: Record<string, unknown>) => Promise<unknown>>();
vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: (cmd: string, args?: Record<string, unknown>) => invoke(cmd, args),
}));

const { default: i18n } = await import("../i18n");
await i18n.changeLanguage("fr");
const { useLibrary } = await import("../store/library");
const { MediaGrid } = await import("./MediaGrid");

const photo = (id: number, burstId: number | null, burstSize: number) => ({
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

// jsdom ne fait pas défiler.
Element.prototype.scrollIntoView = () => {};

describe("grille : rafales", () => {
  beforeEach(() => {
    invoke
      .mockReset()
      .mockResolvedValue([photo(2, 9, 3), photo(1, 9, 3), photo(3, 9, 3), photo(4, null, 1)]);
    useLibrary.setState({
      libraryId: 1,
      media: [photo(2, 9, 3), photo(4, null, 1)],
      selectedId: 4,
      selectedIds: [4],
      expanded: [],
      filter: {},
      trash: false,
      view: "grid",
    });
  });

  it("badge sur la photo du dessus ; un clic déplie sans changer la sélection", async () => {
    render(<MediaGrid />);
    const badge = screen.getByTitle("Déplier la rafale (3 photos) (S)");
    expect(badge).toHaveTextContent("3");
    // Une photo seule n'a pas de badge.
    expect(screen.getAllByTitle(/rafale/)).toHaveLength(1);
    await act(async () => fireEvent.click(badge));
    expect(useLibrary.getState().expanded).toEqual([2]);
    expect(useLibrary.getState().selectedIds).not.toContain(2);
    // Dépliée : les trois photos sont reliées.
    expect(document.querySelectorAll(".media-cell[data-burst]")).toHaveLength(3);
    // Le badge reste sur la photo du dessus seulement.
    expect(document.querySelectorAll(".burst-badge")).toHaveLength(1);
  });
});
