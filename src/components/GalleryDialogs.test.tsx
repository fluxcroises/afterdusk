import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn<(cmd: string, args?: Record<string, unknown>) => Promise<unknown>>();
vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: (cmd: string, args?: Record<string, unknown>) => invoke(cmd, args),
}));
const open = vi.fn<() => Promise<string | null>>();
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: () => open() }));

const { default: i18n } = await import("../i18n");
await i18n.changeLanguage("fr");
const { useGallery, DEFAULT_GALLERY_OPTIONS } = await import("../store/gallery");
const { useLibrary } = await import("../store/library");
const { GalleryDialog, SelectionImportDialog } = await import("./GalleryDialogs");

const photo = (id: number, fileName: string, extra: object = {}) => ({
  id,
  albumId: 9,
  fileName,
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
  companionOf: null,
  ...extra,
});

const album = {
  id: 9,
  name: "Église",
  folder: "2026/Église",
  startDate: "2026-06-13",
  endDate: "2026-06-13",
  count: 2,
  place: null,
  coverMediaId: null,
  coverChosen: false,
  clientId: 3,
  clientName: "Famille Dupont",
  sessionTypeId: 1,
};

const match = {
  galleryId: 1,
  libraryId: 1,
  code: "AD-7KQ3F9MZ",
  title: "Église",
  clientId: 3,
  clientName: "Famille Dupont",
  albumId: 9,
  mediaIds: [1, 5],
  unknown: [7],
};

function reply(cmd: string): unknown {
  switch (cmd) {
    case "identity_get":
      return { name: "Lewis Bedar", copyright: "", website: "", email: "" };
    case "session_types_list":
      return [{ id: 1, key: "wedding", name: null }];
    case "clients_list":
      return [];
    case "gallery_create":
      return { code: "AD-7KQ3F9MZ", folder: "D:/Galeries/Église", photos: 2, missing: 0 };
    case "gallery_read_selection":
      return match;
    case "gallery_apply_selection":
      return { flagged: 2, collectionId: 14 };
    default:
      return null;
  }
}

describe("Livrer > Galerie web", () => {
  beforeEach(() => {
    invoke.mockReset().mockImplementation((cmd) => Promise.resolve(reply(cmd)));
    open.mockReset().mockResolvedValue("D:/Galeries");
    useLibrary.setState({
      libraryId: 1,
      albums: [album],
      albumId: 9,
      collectionId: null,
      selectedIds: [],
      media: [
        photo(1, "DSC_0001.NEF"),
        photo(2, "DSC_0001.JPG", { companionOf: 1 }),
        photo(5, "DSC_0005.NEF"),
      ],
    } as never);
    useGallery.setState({ open: true, options: DEFAULT_GALLERY_OPTIONS, result: null });
  });

  it("crée la galerie de l'album avec le client, la séance et les textes de la page", async () => {
    render(<GalleryDialog />);
    await act(async () => {});
    expect(screen.getByLabelText("Titre")).toHaveValue("Église");
    // Sans courriel dans l'identité, le client renverra un fichier.
    expect(screen.getByText(/Sans courriel dans Réglages/)).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Thème"), { target: { value: "dark" } });
    fireEvent.click(screen.getByLabelText("Les visiteurs peuvent télécharger les photos"));
    fireEvent.change(screen.getByLabelText("Texte d'accueil"), {
      target: { value: "Merci pour cette belle journée." },
    });
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Créer…" })));
    const request = invoke.mock.calls.find(([c]) => c === "gallery_create")?.[1]?.request as {
      [key: string]: unknown;
      texts: Record<string, string>;
    };
    expect(request).toMatchObject({
      libraryId: 1,
      title: "Église",
      subtitle: expect.stringMatching(/^Famille Dupont · Mariage · .+ · Lewis Bedar$/),
      intro: "Merci pour cette belle journée.",
      theme: "dark",
      allowDownload: true,
      selection: true,
      albumId: 9,
      clientId: 3,
      mediaIds: [1, 5],
      destination: "D:/Galeries",
    });
    expect(request.texts).toMatchObject({ lang: "fr", photo: "Photo {n}" });
    expect(screen.getByText("Galerie créée : 2 photos, code AD-7KQ3F9MZ.")).toBeInTheDocument();
    // Le thème reste pour la galerie suivante.
    expect(useGallery.getState().options.theme).toBe("dark");
  });

  it("relit une sélection collée, retient les photos et ouvre la collection", async () => {
    const showCollection = vi.fn(() => Promise.resolve());
    useLibrary.setState({ showCollection } as never);
    useGallery.setState({ importOpen: true, text: "", found: null, importError: null });
    render(<SelectionImportDialog />);
    const mail = "Ma sélection\n[AD-7KQ3F9MZ: 1 2 7]";
    await act(async () =>
      fireEvent.change(screen.getByLabelText("Texte de la sélection"), {
        target: { value: mail },
      }),
    );
    expect(screen.getByRole("status")).toHaveTextContent(
      "Galerie « Église » : 2 photos choisies · Famille Dupont",
    );
    expect(screen.getByText("Le numéro 7 n'existe pas dans cette galerie.")).toBeInTheDocument();
    await act(async () =>
      fireEvent.click(screen.getByRole("button", { name: "Retenir et créer la collection" })),
    );
    expect(invoke).toHaveBeenCalledWith("gallery_apply_selection", {
      text: mail,
      collectionName: "Sélection – Famille Dupont",
    });
    expect(showCollection).toHaveBeenCalledWith(14);
    expect(useGallery.getState().importOpen).toBe(false);
  });
});
