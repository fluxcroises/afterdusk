import { fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import i18n from "../i18n";
import { useImport } from "../store/import";
import { useLibrary } from "../store/library";
import { useReminders } from "../store/maintenance";

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
    // Sans dossier choisi (pas de dossier Images hors de l'application), pas de suite.
    expect(screen.getByRole("button", { name: "Suivant" })).toBeDisabled();
    // Sans bibliothèque, les actions de l'en-tête n'ont pas lieu d'être.
    expect(screen.queryByRole("button", { name: "Importer" })).toBeNull();
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

  it("un entretien échu s'annonce et ouvre la fiche du matériel", () => {
    useImport.setState({ libraries: [library(1, "Photos")], libraryId: 1 });
    useReminders.setState({
      // Pas de cœur dans les tests : les rappels sont posés tels quels.
      load: () => Promise.resolve(),
      reminders: [
        {
          gearId: 4,
          gearName: "Nikon Z 6",
          kind: "sensor",
          lastDate: "2026-01-10",
          dueDate: "2026-07-10",
          due: true,
        },
        {
          gearId: 5,
          gearName: "NIKKOR Z 24-70mm f/4 S",
          kind: "service",
          lastDate: "2026-01-10",
          dueDate: "2027-01-10",
          due: false,
        },
      ],
    });
    render(<WelcomeApp />);
    expect(
      screen.getByText("Nikon Z 6 : Nettoyage du capteur, à faire depuis le 10 juil. 2026."),
    ).toBeInTheDocument();
    expect(screen.queryByText(/NIKKOR/)).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Ouvrir la fiche" }));
    expect(openMain).toHaveBeenCalledWith({ screen: "gear", gearId: 4 });
  });
});
