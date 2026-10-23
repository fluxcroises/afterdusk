import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn<(cmd: string, args?: Record<string, unknown>) => Promise<unknown>>();
vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: (cmd: string, args?: Record<string, unknown>) => invoke(cmd, args),
}));

const { default: i18n } = await import("../i18n");
await i18n.changeLanguage("fr");
const { useImport } = await import("../store/import");
const { useLibrary } = await import("../store/library");
const { useNavigation } = await import("../store/navigation");
const { ImportGrid } = await import("./ImportGrid");

const file = (name: string, mediaId: number | null) => ({
  path: `E:/DCIM/${name}`,
  fileName: name,
  size: 1,
  format: "image" as const,
  capturedAt: "2026-06-21T11:55:00",
  camera: "X-T50",
  alreadyImported: mediaId !== null,
  mediaId,
  albumId: mediaId === null ? null : 4,
  companions: [],
});

describe("Importer : photos de la source", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue([]);
    useNavigation.setState({ screen: "import" });
    useImport.setState({
      source: {
        root: "E:/DCIM",
        ignored: 0,
        days: [
          { day: "2026-06-21", files: [file("DSCF1667.JPG", 12), file("DSCF1679.JPG", null)] },
        ],
      },
      selected: new Set(["E:/DCIM/DSCF1679.JPG"]),
      running: false,
    });
  });

  it("une photo déjà importée s'ouvre dans la visionneuse de Trier", async () => {
    const showAlbum = vi.spyOn(useLibrary.getState(), "showAlbum").mockResolvedValue();
    const select = vi.spyOn(useLibrary.getState(), "select").mockResolvedValue();
    render(<ImportGrid />);
    await act(async () => fireEvent.click(screen.getByTitle("Déjà importée : ouvrir dans Trier")));
    expect(useNavigation.getState().screen).toBe("cull");
    expect(showAlbum).toHaveBeenCalledWith(4);
    expect(select).toHaveBeenCalledWith(12);
    expect(useLibrary.getState().view).toBe("loupe");
    // Une photo à importer reste une case à cocher.
    expect(screen.getByRole("checkbox", { name: /DSCF1679/ })).toBeChecked();
  });
});
