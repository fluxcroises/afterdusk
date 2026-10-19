import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn<(cmd: string, args?: Record<string, unknown>) => Promise<unknown>>();
vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: (cmd: string, args?: Record<string, unknown>) => invoke(cmd, args),
}));

const { default: i18n } = await import("../i18n");
await i18n.changeLanguage("fr");
const { useImport } = await import("../store/import");
const { ImportSettings, LibrarySettings } = await import("./Pages");

const library = {
  id: 1,
  name: "Photos",
  rootPath: "D:/Photos/Afterdusk",
  backupPath: "E:/Sauvegarde",
  folderTemplate: "{yyyy}/{yyyy}-{mm}-{dd} {album}",
  renameTemplate: null,
  isOnline: true,
};

describe("réglages de la bibliothèque", () => {
  beforeEach(() => {
    invoke.mockReset().mockResolvedValue(library);
    useImport.setState({ libraries: [library], libraryId: 1 });
  });

  it("Bibliothèques : emplacement et sauvegarde, que l'on peut retirer", async () => {
    render(<LibrarySettings />);
    expect(screen.getByText("D:/Photos/Afterdusk")).toBeInTheDocument();
    expect(screen.getByText("E:/Sauvegarde")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Retirer" }));
    await vi.waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("library_set_backup", { id: 1, backup: null }),
    );
  });

  it("Import : un modèle choisi s'enregistre pour la bibliothèque", async () => {
    render(<ImportSettings />);
    const save = screen.getByRole("button", { name: "Enregistrer" });
    expect(save).toBeDisabled();
    fireEvent.click(screen.getByRole("radio", { name: /Année \/ mois \/ jour/ }));
    expect(save).toBeEnabled();
    fireEvent.click(save);
    await vi.waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("library_set_templates", {
        id: 1,
        folderTemplate: "{yyyy}/{mm}/{dd}",
        renameTemplate: null,
      }),
    );
  });
});
