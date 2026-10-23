import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn<(cmd: string, args?: Record<string, unknown>) => Promise<unknown>>();
vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  isTauri: () => true,
  invoke: (cmd: string, args?: Record<string, unknown>) => invoke(cmd, args),
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: () => Promise.resolve(() => undefined) }));

const { default: i18n } = await import("../i18n");
await i18n.changeLanguage("fr");
const { useUpdates } = await import("../store/updates");
const { UpdateBanner } = await import("./UpdateBanner");

describe("mises à jour", () => {
  beforeEach(() => {
    invoke.mockReset();
    useUpdates.setState({ available: null, dismissed: false, progress: null, error: null });
  });

  it("rien à proposer : pas de bandeau", async () => {
    invoke.mockResolvedValue(null);
    render(<UpdateBanner />);
    await act(async () => {});
    expect(invoke).toHaveBeenCalledWith("update_check", undefined);
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("nouvelle version proposée, installée seulement sur clic", async () => {
    invoke.mockImplementation((cmd) =>
      Promise.resolve(cmd === "update_check" ? { version: "0.5.1", notes: null } : null),
    );
    render(<UpdateBanner />);
    await act(async () => {});
    expect(screen.getByRole("status")).toHaveTextContent("Afterdusk 0.5.1 est disponible.");
    expect(invoke).not.toHaveBeenCalledWith("update_install", undefined);
    await act(async () =>
      fireEvent.click(screen.getByRole("button", { name: "Installer et redémarrer" })),
    );
    expect(invoke).toHaveBeenCalledWith("update_install", undefined);
  });

  it("Plus tard masque le bandeau", async () => {
    invoke.mockResolvedValue({ version: "0.5.1", notes: null });
    render(<UpdateBanner />);
    await act(async () => {});
    fireEvent.click(screen.getByRole("button", { name: "Plus tard" }));
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("échec de l'installation : message, version actuelle gardée", async () => {
    invoke.mockImplementation((cmd) =>
      cmd === "update_check"
        ? Promise.resolve({ version: "0.5.1", notes: null })
        : Promise.reject({ code: "updateFailed", detail: "signature" }),
    );
    render(<UpdateBanner />);
    await act(async () => {});
    await act(async () =>
      fireEvent.click(screen.getByRole("button", { name: "Installer et redémarrer" })),
    );
    expect(screen.getByRole("alert")).toHaveTextContent("La mise à jour n'a pas pu s'installer.");
  });

  it("la case des Réglages enregistre le choix", async () => {
    invoke.mockResolvedValue(null);
    await useUpdates.getState().setEnabled(false);
    expect(invoke).toHaveBeenCalledWith("updates_enabled_set", { enabled: false });
    expect(useUpdates.getState().enabled).toBe(false);
  });
});
