import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import App from "./App";
import i18n from "./i18n";
import { useNavigation } from "./store/navigation";

describe("App", () => {
  beforeEach(async () => {
    await i18n.changeLanguage("fr");
    useNavigation.setState({ screen: "import" });
  });

  it("affiche les quatre étapes du flux", () => {
    render(<App />);
    for (const label of ["Importer", "Trier", "Retoucher", "Livrer"]) {
      expect(screen.getByRole("button", { name: new RegExp(label) })).toBeInTheDocument();
    }
  });

  it("change d'écran au clic sur une étape", () => {
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: /Trier/ }));
    expect(screen.getByRole("heading", { name: "Trier" })).toBeInTheDocument();
  });

  it("bascule en anglais", async () => {
    await i18n.changeLanguage("en");
    render(<App />);
    expect(screen.getByRole("button", { name: /Library/ })).toBeInTheDocument();
  });
});
