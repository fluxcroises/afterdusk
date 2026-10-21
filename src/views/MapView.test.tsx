import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn<(cmd: string, args?: Record<string, unknown>) => Promise<unknown>>();
vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: (cmd: string, args?: Record<string, unknown>) => invoke(cmd, args),
}));
// Pas de WebGL dans les tests : la carte elle-même est remplacée.
vi.mock("../components/WorldMap", () => ({ WorldMap: () => <div data-testid="carte" /> }));

const { default: i18n } = await import("../i18n");
await i18n.changeLanguage("fr");
const { useLibrary } = await import("../store/library");
const { useMap } = await import("../store/map");
const { useNavigation } = await import("../store/navigation");
const { MapView } = await import("./MapView");

const cabourg = {
  albumId: 3,
  name: "Sortie à Cabourg",
  startDate: "2026-07-14",
  endDate: "2026-07-14",
  latitude: 49.29,
  longitude: -0.11,
  located: 12,
  total: 15,
  coverMediaId: 30,
  city: "Cabourg",
  country: "FR",
};

describe("écran Carte", () => {
  beforeEach(() => {
    invoke.mockReset().mockImplementation((cmd) =>
      Promise.resolve(
        cmd === "geo_map"
          ? {
              albums: [cabourg],
              media: [
                { id: 30, albumId: 3, latitude: 49.29, longitude: -0.11 },
                { id: 31, albumId: 3, latitude: 43.3, longitude: 5.37 },
              ],
            }
          : cmd === "geo_album_tracks"
            ? [[[-0.11, 49.29]]]
            : cmd === "geo_locate_manual"
              ? { located: 2 }
              : [],
      ),
    );
    useLibrary.setState({ libraryId: 1, selectedId: 30, selectedIds: [30, 31] });
    useMap.setState({ albums: [], media: [], selectedAlbumId: null, area: null, mode: "browse" });
  });

  it("liste les albums localisés et montre l'album choisi", async () => {
    await act(async () => render(<MapView />));
    expect(invoke).toHaveBeenCalledWith("geo_map", { libraryId: 1 });
    expect(screen.getByText("1 album · 2 photos localisées")).toBeInTheDocument();
    await act(async () =>
      fireEvent.click(screen.getByRole("button", { name: /Sortie à Cabourg/ })),
    );
    expect(invoke).toHaveBeenCalledWith("geo_album_tracks", {
      libraryId: 1,
      startDate: "2026-07-14",
      endDate: "2026-07-14",
    });
    expect(screen.getByText("Lieu").nextSibling).toHaveTextContent("Cabourg, France");
    expect(screen.getByText("12 sur 15")).toBeInTheDocument();
    expect(screen.getByText("Traces GPX").nextSibling).toHaveTextContent("1");
  });

  it("zone tracée : nombre de photos, puis filtre de la Bibliothèque", async () => {
    const setFilter = vi.spyOn(useLibrary.getState(), "setFilter").mockResolvedValue();
    const showAlbum = vi.spyOn(useLibrary.getState(), "showAlbum").mockResolvedValue();
    await act(async () => render(<MapView />));
    act(() => useMap.getState().setArea([-1, 48, 1, 50]));
    expect(screen.getByText("Photos dans la zone").nextSibling).toHaveTextContent("1");
    await act(async () =>
      fireEvent.click(screen.getByRole("button", { name: "Voir la photo dans la Bibliothèque" })),
    );
    expect(useNavigation.getState().screen).toBe("library");
    expect(showAlbum).toHaveBeenCalledWith(null);
    expect(setFilter).toHaveBeenCalledWith({
      area: [-1_000_000, 48_000_000, 1_000_000, 50_000_000],
    });
  });

  it("placer la sélection d'un clic, sans remplacer", async () => {
    await act(async () => render(<MapView />));
    const place = screen.getByRole("button", { name: "Placer 2 photos" });
    fireEvent.click(place);
    expect(place).toHaveAttribute("aria-pressed", "true");
    expect(
      screen.getByText("Clic : placer les 2 photos sélectionnées · Échap : annuler"),
    ).toBeInTheDocument();
    await act(() => useMap.getState().place(48.85, 2.35));
    expect(invoke).toHaveBeenCalledWith("geo_locate_manual", {
      ids: [30, 31],
      latitude: 48.85,
      longitude: 2.35,
      replace: false,
    });
    expect(useMap.getState().mode).toBe("browse");
    expect(screen.getByText("2 photos localisées")).toBeInTheDocument();
  });
});
