import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import i18n from "../i18n";
import { useImport } from "../store/import";

const openMain = vi.fn();
vi.mock("../windows", async (original) => ({
  ...(await original<typeof import("../windows")>()),
  openMain: (...args: unknown[]) => openMain(...args),
}));

const { WelcomeApp } = await import("./WelcomeApp");

describe("fenêtre d'accueil", () => {
  beforeEach(async () => {
    await i18n.changeLanguage("fr");
    openMain.mockReset();
    useImport.setState({ libraries: [], libraryId: null, resumable: [], running: false });
  });

  it("salue, invite à créer une bibliothèque et présente le parcours", () => {
    render(<WelcomeApp />);
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent(/Bonjour|Bonsoir/);
    expect(screen.getByRole("heading", { name: "Pour commencer" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Le parcours Afterdusk" })).toBeInTheDocument();
  });

  it("une carte du parcours ouvre la fenêtre principale sur l'étape", () => {
    render(<WelcomeApp />);
    fireEvent.click(screen.getByRole("button", { name: /^Trier/ }));
    expect(openMain).toHaveBeenCalledWith({ screen: "cull" });
  });

  it("propose de reprendre un import interrompu", () => {
    useImport.setState({
      resumable: [
        {
          id: 4,
          libraryId: 1,
          sourceLabel: "EOS_DIGITAL",
          sourcePath: "E:",
          startedAt: "",
          remaining: 3,
          total: 9,
        },
      ],
    });
    render(<WelcomeApp />);
    fireEvent.click(screen.getByRole("button", { name: "Reprendre" }));
    expect(openMain).toHaveBeenCalledWith({ screen: "import", resumeImportId: 4 });
  });
});
