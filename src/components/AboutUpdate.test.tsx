import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn<(cmd: string) => Promise<unknown>>();
vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  isTauri: () => true,
  invoke: (cmd: string) => invoke(cmd),
}));

vi.mock("@tauri-apps/api/event", () => ({ listen: () => Promise.resolve(() => undefined) }));

const { default: i18n } = await import("../i18n");
await i18n.changeLanguage("fr");
const { useAbout } = await import("../store/about");
const { useUpdates } = await import("../store/updates");
const { AboutDialog } = await import("./AboutDialog");

const answer = (check: unknown) => (cmd: string) =>
  cmd === "update_check_now"
    ? check instanceof Error
      ? Promise.reject(check)
      : Promise.resolve(check)
    : Promise.resolve(null);

describe("À propos : rechercher les mises à jour", () => {
  beforeEach(() => {
    invoke.mockReset();
    useAbout.setState({ open: true });
    useUpdates.setState({ available: null, progress: null, manual: null, dismissed: false });
  });

  it("à jour", async () => {
    invoke.mockImplementation(answer(null));
    render(<AboutDialog />);
    await act(async () =>
      fireEvent.click(screen.getByRole("button", { name: "Rechercher les mises à jour" })),
    );
    expect(invoke).toHaveBeenCalledWith("update_check_now");
    expect(screen.getByText("Afterdusk est à jour.")).toBeInTheDocument();
  });

  it("nouvelle version : proposée ici et dans le bandeau", async () => {
    invoke.mockImplementation(answer({ version: "0.5.2", notes: null }));
    render(<AboutDialog />);
    await act(async () =>
      fireEvent.click(screen.getByRole("button", { name: "Rechercher les mises à jour" })),
    );
    expect(screen.getByText(/Version 0\.5\.2 disponible\./)).toBeInTheDocument();
    expect(useUpdates.getState().available?.version).toBe("0.5.2");
    await act(async () =>
      fireEvent.click(screen.getByRole("button", { name: "Installer et redémarrer" })),
    );
    expect(invoke).toHaveBeenCalledWith("update_install");
  });

  it("sans réseau : le dit, et propose de réessayer", async () => {
    invoke.mockImplementation(answer(new Error("réseau")));
    render(<AboutDialog />);
    await act(async () =>
      fireEvent.click(screen.getByRole("button", { name: "Rechercher les mises à jour" })),
    );
    expect(screen.getByText(/Recherche impossible/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Réessayer" })).toBeInTheDocument();
  });
});
