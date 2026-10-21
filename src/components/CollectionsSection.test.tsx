import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn<(cmd: string, args?: Record<string, unknown>) => Promise<unknown>>();
vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: (cmd: string, args?: Record<string, unknown>) => invoke(cmd, args),
}));
const confirmWarning = vi.fn(() => Promise.resolve(true));
vi.mock("../ui/confirm", () => ({ confirmWarning: () => confirmWarning() }));

const { default: i18n } = await import("../i18n");
await i18n.changeLanguage("fr");
const { useLibrary } = await import("../store/library");
const { useCollections, sameFilter } = await import("../store/collections");
const { CollectionsSection } = await import("./CollectionsSection");

const fiveAt23 = {
  rating: { op: "exactly" as const, value: 5 },
  focal: [23, 23] as [number, number],
};
const collections = [
  { id: 1, name: "Meilleures photos 2026", kind: "manual", filter: null, count: 12 },
  { id: 2, name: "5 étoiles au 23 mm", kind: "smart", filter: fiveAt23, count: 4 },
];
const photo = (id: number) => ({
  id,
  albumId: 3,
  fileName: `P${id}.RAF`,
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

describe("panneau Collections", () => {
  beforeEach(() => {
    invoke.mockReset().mockImplementation((cmd) => {
      // Le cœur peut renvoyer des champs vides : ils ne filtrent pas.
      if (cmd === "collections_list")
        return Promise.resolve(
          collections.map((c) => (c.filter ? { ...c, filter: { ...c.filter, area: null } } : c)),
        );
      if (cmd === "collection_create") return Promise.resolve(3);
      if (cmd === "media_list") return Promise.resolve([photo(7), photo(8)]);
      return Promise.resolve(null);
    });
    confirmWarning.mockClear();
    useCollections.setState({ collections: [], activeSmartId: null });
    useLibrary.setState({
      libraryId: 1,
      albumId: null,
      collectionId: null,
      trash: false,
      showAlbums: false,
      filter: {},
      media: [],
      selectedId: null,
      selectedIds: [],
      view: "grid",
    });
  });

  it("liste les deux types ; un album intelligent applique son filtre", async () => {
    await act(async () => render(<CollectionsSection scope="library" />));
    const smart = screen.getByRole("button", { name: /5 étoiles au 23 mm\s*intelligent\s*4/ });
    expect(screen.getByRole("button", { name: /Meilleures photos 2026\s*12/ })).toBeInTheDocument();
    // Sans filtre actif, rien à enregistrer.
    expect(
      screen.getByRole("button", { name: "Enregistrer le filtre comme album intelligent…" }),
    ).toBeDisabled();

    await act(async () => fireEvent.click(smart));
    expect(useLibrary.getState().filter).toEqual(fiveAt23);
    expect(invoke).toHaveBeenCalledWith(
      "media_list",
      expect.objectContaining({ albumId: null, filter: fiveAt23 }),
    );
    expect(smart).toHaveAttribute("aria-current", "true");
    // Filtre identique : rien à remplacer ; modifié, l'action apparaît.
    expect(screen.queryByText("Remplacer par le filtre actuel")).toBeNull();
    await act(async () => useLibrary.getState().setFilter({ focal: [35, 35] }));
    await act(async () => fireEvent.click(screen.getByText("Remplacer par le filtre actuel")));
    expect(invoke).toHaveBeenCalledWith("collection_set_filter", {
      id: 2,
      filter: { ...fiveAt23, focal: [35, 35] },
    });
  });

  it("une collection manuelle : ouverture, ajout de la sélection, retrait, suppression", async () => {
    useLibrary.setState({ media: [photo(7), photo(8)], selectedId: 7, selectedIds: [7, 8] });
    await act(async () => render(<CollectionsSection scope="library" />));
    // Sélection de deux photos : bouton d'ajout sur la collection manuelle seulement.
    const add = screen.getByRole("button", {
      name: "Ajouter les 2 photos sélectionnées à « Meilleures photos 2026 »",
    });
    expect(screen.getAllByRole("button", { name: /^Ajouter les/ })).toHaveLength(1);
    await act(async () => fireEvent.click(add));
    expect(invoke).toHaveBeenCalledWith("collection_add_media", { id: 1, mediaIds: [7, 8] });

    await act(async () =>
      fireEvent.click(screen.getByRole("button", { name: /Meilleures photos 2026\s*12/ })),
    );
    expect(useLibrary.getState().collectionId).toBe(1);
    expect(invoke).toHaveBeenCalledWith(
      "media_list",
      expect.objectContaining({ filter: { collectionId: 1 } }),
    );
    useLibrary.setState({ selectedId: 7, selectedIds: [7] });
    await act(async () =>
      fireEvent.click(screen.getByRole("button", { name: "Retirer la photo de la collection" })),
    );
    expect(invoke).toHaveBeenCalledWith("collection_remove_media", { id: 1, mediaIds: [7] });

    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Supprimer" })));
    expect(confirmWarning).toHaveBeenCalled();
    expect(invoke).toHaveBeenCalledWith("collection_delete", { id: 1 });
    // La collection affichée disparaît : retour à toutes les photos.
    expect(useLibrary.getState().collectionId).toBeNull();
  });

  it("crée une collection, et un album intelligent du filtre courant", async () => {
    useLibrary.setState({ filter: { flags: [1], collectionId: 1 } });
    await act(async () => render(<CollectionsSection scope="library" />));
    fireEvent.click(screen.getByRole("button", { name: "Nouvelle collection…" }));
    const field = screen.getByLabelText("Nom de la nouvelle collection");
    fireEvent.change(field, { target: { value: "  Portfolio " } });
    await act(async () => fireEvent.submit(field));
    expect(invoke).toHaveBeenCalledWith("collection_create", {
      libraryId: 1,
      name: "Portfolio",
      filter: null,
    });

    fireEvent.click(
      screen.getByRole("button", { name: "Enregistrer le filtre comme album intelligent…" }),
    );
    const smartField = screen.getByLabelText("Nom du nouvel album intelligent");
    fireEvent.change(smartField, { target: { value: "Retenues" } });
    await act(async () => fireEvent.submit(smartField));
    // La collection affichée ne fait pas partie du filtre enregistré.
    expect(invoke).toHaveBeenCalledWith("collection_create", {
      libraryId: 1,
      name: "Retenues",
      filter: { flags: [1] },
    });

    // Échap referme le champ sans rien créer.
    fireEvent.click(screen.getByRole("button", { name: "Nouvelle collection…" }));
    fireEvent.keyDown(screen.getByLabelText("Nom de la nouvelle collection"), { key: "Escape" });
    expect(screen.queryByLabelText("Nom de la nouvelle collection")).toBeNull();
    expect(within(document.body).queryByText("Aucune collection.")).toBeNull();
  });

  it("compare deux filtres sans tenir compte de l'ordre ni des listes vides", () => {
    expect(sameFilter({ flags: [], rating: fiveAt23.rating }, { rating: fiveAt23.rating })).toBe(
      true,
    );
    expect(sameFilter({ focal: [23, 23] }, { focal: [35, 35] })).toBe(false);
    expect(sameFilter({ collectionId: 4 }, {})).toBe(true);
  });
});
