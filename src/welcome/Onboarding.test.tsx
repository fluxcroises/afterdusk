import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import i18n from "../i18n";

vi.mock("../settings/onboarding", async (original) => ({
  ...(await original<typeof import("../settings/onboarding")>()),
  defaultRoot: () => Promise.resolve("/Users/moi/Pictures/Afterdusk"),
}));

const { Onboarding } = await import("./Onboarding");

describe("assistant de première ouverture : sauvegarde", () => {
  beforeEach(async () => {
    await i18n.changeLanguage("fr");
  });

  it("pose la question ; le dossier n'est demandé qu'après un oui", async () => {
    render(<Onboarding />);
    await act(async () => {});
    fireEvent.click(screen.getByRole("button", { name: "Suivant" }));
    fireEvent.click(screen.getByRole("button", { name: "Suivant" }));
    expect(
      screen.getByRole("heading", { name: "Voulez-vous une sauvegarde ?" }),
    ).toBeInTheDocument();
    // Sans réponse, pas de suite ni de choix de dossier.
    expect(screen.getByRole("button", { name: "Suivant" })).toBeDisabled();
    expect(screen.queryByText("Dossier de sauvegarde")).toBeNull();
    fireEvent.click(screen.getByRole("radio", { name: /^Oui/ }));
    expect(screen.getByText("Dossier de sauvegarde")).toBeInTheDocument();
    // Oui sans dossier choisi : pas encore de suite.
    expect(screen.getByRole("button", { name: "Suivant" })).toBeDisabled();
    fireEvent.click(screen.getByRole("radio", { name: "Non, pas de sauvegarde" }));
    expect(screen.queryByText("Dossier de sauvegarde")).toBeNull();
    expect(screen.getByRole("button", { name: "Suivant" })).toBeEnabled();
    expect(screen.queryByText("Facultatif.")).toBeNull();
  });
});
