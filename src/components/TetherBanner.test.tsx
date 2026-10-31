import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: vi.fn(async () => null),
}));

const { invoke } = await import("@tauri-apps/api/core");
const { default: i18n } = await import("../i18n");
await i18n.changeLanguage("fr");
const { useImport } = await import("../store/import");
const { useTether } = await import("../store/tether");
const { TetherBanner } = await import("./TetherBanner");

const idle = {
  camera: "FUJIFILM X-H2",
  active: false,
  connected: false,
  libraryId: null,
  albumName: "",
  folderId: null,
  received: 0,
  remaining: null,
  leftOnCard: 0,
  failed: [],
};

const running = {
  ...idle,
  active: true,
  connected: true,
  libraryId: 1,
  albumName: "Studio",
  folderId: 3,
  received: 12,
  remaining: 37,
};

describe("séance reliée au boîtier", () => {
  beforeEach(() => {
    useImport.setState({
      libraries: [
        {
          id: 1,
          name: "Photos",
          rootPath: "E:\\Photos",
          backupPath: "F:\\Copie",
          folderTemplate: "{yyyy}/{album}",
          renameTemplate: null,
          isOnline: true,
        },
      ],
      libraryId: 1,
    });
    useTether.setState({ dismissed: null, starting: false, lastCamera: null, error: null });
    useTether.getState().onStatus({ ...idle, camera: null });
  });

  it("rien sans boîtier", () => {
    render(<TetherBanner />);
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("boîtier relié : la fenêtre de démarrage part avec l'album choisi", async () => {
    act(() => useTether.getState().onStatus(idle));
    render(<TetherBanner />);
    expect(screen.getByRole("status")).toHaveTextContent("FUJIFILM X-H2 relié.");
    fireEvent.click(screen.getByRole("button", { name: "Démarrer la séance" }));
    expect(screen.getByRole("dialog")).toHaveTextContent("Séance reliée au X-H2");
    const album = screen.getByRole("textbox");
    expect((album as HTMLInputElement).value).toMatch(/^X-H2 · /);
    fireEvent.change(album, { target: { value: "Studio" } });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Démarrer" }));
    });
    expect(invoke).toHaveBeenCalledWith("tether_start", {
      libraryId: 1,
      albumName: "Studio",
      settings: { backup: true, keywords: [], rights: null },
    });
  });

  it("séance en cours, carte presque pleine, débranchement", () => {
    act(() => useTether.getState().onStatus(running));
    render(<TetherBanner />);
    expect(screen.getByRole("status")).toHaveTextContent(
      "X-H2 · album « Studio » · 12 photos reçues · place sur la carte : 37 photos",
    );
    act(() => useTether.getState().onStatus({ ...running, remaining: 4 }));
    expect(screen.getByRole("status")).toHaveTextContent("plus que 4 photos sur la carte");
    act(() => useTether.getState().onStatus({ ...running, camera: null, connected: false }));
    expect(screen.getByRole("status")).toHaveTextContent(
      "X-H2 débranché · 12 photos reçues Rebranchez-le pour continuer la séance.",
    );
  });

  it("rebranché : les photos restées sur la carte sont signalées une fois", () => {
    act(() => useTether.getState().onStatus(running));
    render(<TetherBanner />);
    act(() => useTether.getState().onStatus({ ...running, leftOnCard: 2 }));
    expect(screen.getByRole("alert")).toHaveTextContent(
      "Au moins 2 photos sont restées sur la carte",
    );
    fireEvent.click(screen.getByRole("button", { name: "Compris" }));
    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.getByRole("status")).toHaveTextContent("12 photos reçues");
  });
});
