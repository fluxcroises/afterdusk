import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn<(cmd: string, args?: Record<string, unknown>) => Promise<unknown>>();
vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: (cmd: string, args?: Record<string, unknown>) => invoke(cmd, args),
}));
const save = vi.fn<() => Promise<string | null>>();
vi.mock("@tauri-apps/plugin-dialog", () => ({ save: () => save() }));

const { default: i18n } = await import("../i18n");
await i18n.changeLanguage("fr");
const { useContactSheet, DEFAULT_OPTIONS } = await import("../store/contactSheet");
const { useLibrary } = await import("../store/library");
const { ContactSheetDialog } = await import("./ContactSheetDialog");

function photo(id: number, fileName: string, extra: object = {}) {
  return {
    id,
    albumId: 1,
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
  };
}

describe("Livrer > Planche contact…", () => {
  beforeEach(() => {
    invoke
      .mockReset()
      .mockImplementation((cmd) =>
        Promise.resolve(
          cmd === "identity_get"
            ? { name: "Lewis Bedar", copyright: "", website: "" }
            : cmd === "identity_logo"
              ? null
              : undefined,
        ),
      );
    save.mockReset().mockResolvedValue("D:/Livraisons/planche.pdf");
    useLibrary.setState({
      albums: [],
      albumId: null,
      collectionId: null,
      selectedIds: [],
      media: [
        photo(1, "DSC_0001.NEF"),
        photo(2, "DSC_0001.JPG", { companionOf: 1 }),
        photo(3, "DSC_0002.NEF"),
        photo(-4, "DSC_0002.NEF", { copyOf: 3, version: "Noir et blanc" }),
      ],
    } as never);
    useContactSheet.setState({ open: true, options: DEFAULT_OPTIONS, error: null });
  });

  it("met les photos de la grille sans les fichiers du dessous, avec les réglages choisis", async () => {
    render(<ContactSheetDialog />);
    // Le nom du photographe arrive de la base.
    await act(async () => {});
    expect(screen.getByText("Toutes les photos · 3 photos")).toBeInTheDocument();
    // Sans logo, ni en-tête ni filigrane logo.
    expect(screen.getByLabelText("Logo en tête de page")).toBeDisabled();
    fireEvent.change(screen.getByLabelText("Colonnes"), { target: { value: "6" } });
    fireEvent.change(screen.getByLabelText("Orientation"), { target: { value: "landscape" } });
    fireEvent.change(screen.getByLabelText("Filigrane"), { target: { value: "text" } });
    expect(screen.getByDisplayValue("© Lewis Bedar")).toBeInTheDocument();
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Enregistrer…" })));
    expect(invoke).toHaveBeenCalledWith("contact_sheet_export", {
      path: "D:/Livraisons/planche.pdf",
      sheet: expect.objectContaining({
        columns: 6,
        paper: "a4",
        orientation: "landscape",
        headerLogo: false,
        watermark: { kind: "text", text: "© Lewis Bedar" },
        title: "Toutes les photos",
        subtitle: "Lewis Bedar · 3 photos",
        footer: "Page {page} sur {pages}",
        items: [
          { mediaId: 1, caption: "DSC_0001.NEF" },
          { mediaId: 3, caption: "DSC_0002.NEF" },
          { mediaId: -4, caption: "DSC_0002.NEF · Noir et blanc" },
        ],
      }),
    });
    expect(useContactSheet.getState().open).toBe(false);
    // Les réglages restent pour la planche suivante.
    expect(useContactSheet.getState().options.columns).toBe(6);
  });

  it("ne garde que la sélection quand plusieurs photos sont choisies", async () => {
    useLibrary.setState({ selectedIds: [3, -4] } as never);
    render(<ContactSheetDialog />);
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Enregistrer…" })));
    const sheet = invoke.mock.calls.find(([c]) => c === "contact_sheet_export")?.[1]?.sheet as {
      items: unknown[];
      subtitle: string;
    };
    expect(sheet.items).toHaveLength(2);
    expect(sheet.subtitle).toContain("sélection");
  });
});
