import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import config from "../../src-tauri/tauri.conf.json";
import macos from "../../src-tauri/tauri.macos.conf.json";
import windows from "../../src-tauri/tauri.windows.conf.json";

vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  isTauri: () => true,
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    isMaximized: () => Promise.resolve(false),
    onResized: () => Promise.resolve(() => undefined),
  }),
}));

const { default: i18n } = await import("../i18n");
await i18n.changeLanguage("fr");
const { WindowControls } = await import("./WindowControls");

describe("fenêtre d'accueil à taille fixe", () => {
  // Les fichiers propres à Windows et à macOS remplacent la liste entière des
  // fenêtres : chacun doit le dire.
  it.each([
    ["commune", config],
    ["Windows", windows],
    ["macOS", macos],
  ])("ni agrandie ni redimensionnée (configuration %s)", (_, conf) => {
    const welcome = conf.app.windows.find((w) => w.label === "welcome");
    expect(welcome).toMatchObject({ resizable: false, maximizable: false });
  });

  it("sans bouton Agrandir quand la fenêtre ne s'agrandit pas", () => {
    render(<WindowControls maximizable={false} />);
    expect(screen.getByRole("button", { name: "Réduire" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Agrandir" })).toBeNull();
    expect(screen.getByRole("button", { name: "Fermer" })).toBeInTheDocument();
  });
});
