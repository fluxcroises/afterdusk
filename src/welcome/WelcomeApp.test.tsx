import { fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import i18n from "../i18n";
import { useImport } from "../store/import";
import { useLibrary } from "../store/library";

const openMain = vi.fn();
vi.mock("../windows", async (original) => ({
  ...(await original<typeof import("../windows")>()),
  openMain: (...args: unknown[]) => openMain(...args),
}));

const { WelcomeApp } = await import("./WelcomeApp");

const library = (id: number, name: string) => ({
  id,
  name,
  rootPath: `D:\\${name}`,
  backupPath: null,
  folderTemplate: "",
  renameTemplate: null,
  isOnline: true,
});

describe("fenêtre d'accueil", () => {
  beforeEach(async () => {
    await i18n.changeLanguage("fr");
    openMain.mockReset();
    useImport.setState({ libraries: [], libraryId: null, resumable: [], running: false });
    useLibrary.setState({ albums: [] });
  });

  it("sans bibliothèque : salue et ouvre l'assistant de première ouverture", () => {
    render(<WelcomeApp />);
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent(/Bonjour|Bonsoir/);
    expect(screen.getByRole("heading", { name: "Où ranger vos photos ?" })).toBeInTheDocument();
    // Quatre étapes, la première en cours.
    expect(
      within(screen.getByRole("list"))
        .getAllByRole("listitem")
        .map((li) => li.textContent),
    ).toEqual(["1Bibliothèque", "2Sauvegarde", "3Rangement", "4Identité"]);
    // Récapitulatif des choix, rempli au fil des étapes.
    const summary = screen.getByRole("complementary", { name: "Votre bibliothèque" });
    expect(summary).toHaveTextContent("Photos");
    expect(summary).toHaveTextContent("À choisir");
    expect(summary).toHaveTextContent("Pas de sauvegarde pour l'instant");
    expect(summary).toHaveTextContent("Non renseignée");
    // Sans dossier choisi (pas de dossier Images hors de l'application), pas de suite.
    expect(screen.getByRole("button", { name: "Suivant" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Importer" })).toBeDisabled();
  });

  it("ne reprend pas les onglets d'Affinity : la photo sert d'en-tête", () => {
    useImport.setState({ libraries: [library(1, "Photos")], libraryId: 1 });
    render(<WelcomeApp />);
    expect(screen.queryByRole("tablist")).toBeNull();
    expect(screen.getByRole("heading", { name: "Albums" })).toBeInTheDocument();
  });

  it("le bouton principal ouvre Importer dans la fenêtre principale", () => {
    useImport.setState({ libraries: [library(1, "Photos")], libraryId: 1 });
    render(<WelcomeApp />);
    fireEvent.click(screen.getByRole("button", { name: "Importer" }));
    expect(openMain).toHaveBeenCalledWith({ screen: "import" });
  });

  it("la liste des albums ouvre l'album dans Trier", () => {
    useImport.setState({ libraries: [library(1, "Photos")], libraryId: 1 });
    useLibrary.setState({
      albums: [
        {
          id: 9,
          name: "Saint-Malo",
          folder: "2026/2026-10-18 Saint-Malo",
          startDate: "2026-10-18",
          endDate: "2026-10-18",
          count: 98,
          coverMediaId: null,
          coverChosen: false,
          place: null,
        },
      ],
    });
    render(<WelcomeApp />);
    fireEvent.click(screen.getByRole("button", { name: /Saint-Malo/ }));
    expect(openMain).toHaveBeenCalledWith({ screen: "cull", albumId: 9 });
  });

  it("change de bibliothèque depuis l'en-tête", () => {
    useImport.setState({
      libraries: [library(1, "Photos"), library(2, "Clients")],
      libraryId: 1,
    });
    render(<WelcomeApp />);
    fireEvent.click(screen.getByRole("button", { name: /Photos/ }));
    fireEvent.click(screen.getByRole("menuitemradio", { name: /Clients/ }));
    expect(useImport.getState().libraryId).toBe(2);
  });

  it("propose de reprendre un import interrompu", () => {
    useImport.setState({
      libraries: [library(1, "Photos")],
      libraryId: 1,
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
