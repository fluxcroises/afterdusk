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
const { useImport } = await import("../store/import");
const { ImportSettings } = await import("./Pages");

const library = {
  id: 1,
  name: "Photos",
  rootPath: "D:/Photos/Afterdusk",
  backupPath: "E:/Sauvegarde",
  folderTemplate: "{yyyy}/{yyyy}-{mm}-{dd} {album}",
  renameTemplate: null,
  isOnline: true,
};

const folder = {
  id: 4,
  path: "C:/Users/Lewis/Pictures/X Acquire",
  libraryId: 1,
  libraryName: "Photos",
  albumName: "Séance en cours",
  settings: { rename: false, backup: true, keywords: [], rights: null },
  enabled: true,
};

describe("Réglages > Import : dossiers surveillés", () => {
  let folders: (typeof folder)[];
  beforeEach(() => {
    folders = [];
    invoke.mockReset().mockImplementation((cmd, args) => {
      if (cmd === "watch_folders") return Promise.resolve(folders);
      if (cmd === "watch_folder_add") {
        folders = [{ ...folder, path: args?.path as string }];
        return Promise.resolve(4);
      }
      if (cmd === "auto_import_rules") return Promise.resolve([]);
      return Promise.resolve(null);
    });
    open.mockReset().mockResolvedValue("C:/Users/Lewis/Pictures/X Acquire");
    useImport.setState({ libraries: [library], libraryId: 1 });
  });

  it("ajoute un dossier dans la bibliothèque affichée, puis le suspend", async () => {
    render(<ImportSettings />);
    await act(async () => {});
    expect(screen.getByText(/Aucun dossier/)).toBeInTheDocument();
    await act(async () =>
      fireEvent.click(screen.getByRole("button", { name: "Ajouter un dossier…" })),
    );
    expect(invoke).toHaveBeenCalledWith("watch_folder_add", {
      path: "C:/Users/Lewis/Pictures/X Acquire",
      libraryId: 1,
      albumName: "Séance en cours",
      settings: { backup: true, keywords: [], rights: null },
    });
    expect(screen.getByText("C:/Users/Lewis/Pictures/X Acquire")).toBeInTheDocument();
    await act(async () =>
      fireEvent.click(screen.getByRole("checkbox", { name: "C:/Users/Lewis/Pictures/X Acquire" })),
    );
    expect(invoke).toHaveBeenCalledWith("watch_folder_set_enabled", { id: 4, enabled: false });
  });

  it("change l'album en quittant le champ", async () => {
    folders = [folder];
    render(<ImportSettings />);
    await act(async () => {});
    const field = screen.getByLabelText("Album des photos de C:/Users/Lewis/Pictures/X Acquire");
    fireEvent.change(field, { target: { value: "Studio – Martin" } });
    await act(async () => fireEvent.blur(field));
    expect(invoke).toHaveBeenCalledWith("watch_folder_update", {
      id: 4,
      albumName: "Studio – Martin",
      settings: folder.settings,
    });
  });
});
