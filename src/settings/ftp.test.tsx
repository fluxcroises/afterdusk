import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn<(cmd: string, args?: Record<string, unknown>) => Promise<unknown>>();
vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: (cmd: string, args?: Record<string, unknown>) => invoke(cmd, args),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ save: () => Promise.resolve(null) }));

const { default: i18n } = await import("../i18n");
await i18n.changeLanguage("fr");
const { useImport } = await import("../store/import");
const { FtpReception } = await import("./FtpReception");

const off = {
  enabled: false,
  port: 2121,
  user: "afterdusk",
  password: "k7m2xq9pa4cz",
  allowPlain: false,
  libraryId: null as number | null,
  albumName: "",
};

describe("Réglages > Import : réception FTP", () => {
  let settings: typeof off;
  beforeEach(() => {
    settings = { ...off };
    invoke.mockReset().mockImplementation((cmd, args) => {
      if (cmd === "ftp_get") {
        return Promise.resolve({
          settings,
          status: { running: settings.enabled, error: null },
          address: "192.168.1.42",
          reception: "C:/Afterdusk/reception-ftp",
          fingerprint: settings.enabled ? "AB:CD:EF" : null,
        });
      }
      if (cmd === "ftp_set") settings = args?.settings as typeof settings;
      return Promise.resolve(null);
    });
    useImport.setState({ libraryId: 3 });
  });

  it("désactivée par défaut, puis activée pour la bibliothèque affichée", async () => {
    render(<FtpReception />);
    await act(async () => {});
    expect(screen.getByText(/Désactivée/)).toBeInTheDocument();
    expect(screen.queryByText("k7m2xq9pa4cz")).toBeNull();
    await act(async () =>
      fireEvent.click(
        screen.getByLabelText("Recevoir les photos des boîtiers par FTP sur le réseau local"),
      ),
    );
    expect(invoke).toHaveBeenCalledWith("ftp_set", {
      settings: { ...off, enabled: true, libraryId: 3, albumName: "Réception FTP" },
    });
    // Ce qu'il faut saisir dans le boîtier.
    expect(screen.getByText("En écoute sur le réseau local.")).toBeInTheDocument();
    expect(screen.getByText("192.168.1.42")).toBeInTheDocument();
    expect(screen.getByText("k7m2xq9pa4cz")).toBeInTheDocument();
    expect(screen.getByText("AB:CD:EF")).toBeInTheDocument();
  });

  it("FTP simple : accepté sur demande, avec un avertissement", async () => {
    settings = { ...off, enabled: true, libraryId: 3, albumName: "Réception FTP" };
    render(<FtpReception />);
    await act(async () => {});
    expect(screen.queryByText(/circulent en clair/)).toBeNull();
    await act(async () =>
      fireEvent.click(screen.getByLabelText("Accepter aussi le FTP non chiffré")),
    );
    expect(invoke).toHaveBeenCalledWith("ftp_set", {
      settings: expect.objectContaining({ allowPlain: true }),
    });
    expect(screen.getByText(/circulent en clair/)).toBeInTheDocument();
  });
});
