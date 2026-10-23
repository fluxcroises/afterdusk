import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn<(cmd: string) => Promise<unknown>>(() => Promise.resolve(null));
vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: (cmd: string) => invoke(cmd),
}));

const { default: i18n } = await import("../i18n");
await i18n.changeLanguage("fr");
const { useAbout, systemReport } = await import("../store/about");
const { AboutDialog } = await import("./AboutDialog");

describe("À propos d'Afterdusk", () => {
  beforeEach(() => useAbout.setState({ open: true }));

  it("version, composants et avis de licence ; Licence… affiche la GPL", () => {
    render(<AboutDialog />);
    const dialog = screen.getByRole("dialog", { name: "Afterdusk" });
    expect(dialog).toHaveTextContent("Version 0.5.0");
    expect(dialog).toHaveTextContent("Composants : LibRaw (LGPL 2.1) · Adobe XMP Toolkit (BSD 3)");
    expect(dialog).toHaveTextContent("SQLite (domaine public)");
    expect(dialog).toHaveTextContent("Cabourg, photo © Lewis Bedar");
    expect(dialog).toHaveTextContent("licence publique générale GNU, version 3");
    fireEvent.click(screen.getByRole("button", { name: "Licence…" }));
    expect(dialog).toHaveTextContent("GNU GENERAL PUBLIC LICENSE");
    fireEvent.click(screen.getByRole("button", { name: "OK" }));
    expect(useAbout.getState().open).toBe(false);
  });

  it("le nom de l'association ouvre son site dans le navigateur", () => {
    render(<AboutDialog />);
    const dialog = screen.getByRole("dialog", { name: "Afterdusk" });
    expect(dialog).toHaveTextContent("Un logiciel libre de l'association Flux Croisés.");
    fireEvent.click(screen.getByRole("button", { name: "Flux Croisés" }));
    expect(invoke).toHaveBeenCalledWith("open_publisher_site");
  });

  it("Échap ferme la boîte", () => {
    render(<AboutDialog />);
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(useAbout.getState().open).toBe(false);
  });

  it("informations système, à joindre à un signalement", () => {
    const report = systemReport(
      {
        version: "0.1.0",
        libraw: "0.22.2",
        sqlite: "3.50.0",
        tauri: "2.12.4",
        webview: "141.0",
        os: "windows",
        arch: "x86_64",
        osVersion: "Windows 10.0.26200",
        cpu: "Apple M3 Pro",
      },
      "fr",
    );
    expect(report).toBe(
      "Afterdusk 0.1.0\nWindows 10.0.26200 x86_64\nCPU Apple M3 Pro\nWebView 141.0\nTauri 2.12.4 · LibRaw 0.22.2 · SQLite 3.50.0\nLocale fr",
    );
  });
});
