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
const { useLayout } = await import("../store/layout");
const { FilterBar } = await import("./FilterBar");

describe("barre de filtres : focale", () => {
  beforeEach(() => {
    invoke.mockReset().mockResolvedValue([]);
    useLayout.setState({ filterBar: true });
    useLibrary.setState({
      libraryId: 1,
      filter: {},
      showAlbums: false,
      collectionId: null,
      filterOptions: {
        cameras: [],
        lenses: [],
        focals: [
          { value: 23, count: 40 },
          { value: 56, count: 7 },
        ],
        firstDay: null,
        lastDay: null,
      },
    });
  });

  it("propose les focales présentes et filtre sur l'une d'elles", async () => {
    render(<FilterBar />);
    const select = screen.getByLabelText("Focale");
    expect(screen.getByRole("option", { name: "23 mm (40)" })).toBeInTheDocument();
    await act(async () => fireEvent.change(select, { target: { value: "23" } }));
    expect(useLibrary.getState().filter.focal).toEqual([23, 23]);
    expect(invoke).toHaveBeenCalledWith(
      "media_list",
      expect.objectContaining({ filter: { focal: [23, 23] } }),
    );
    // Un filtre actif : Réinitialiser apparaît et retire la focale.
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Réinitialiser" })));
    expect(useLibrary.getState().filter).toEqual({});
  });
});
