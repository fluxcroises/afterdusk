import { fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import App from "./App";
import { handleMenuShortcut } from "./menu";
import i18n from "./i18n";
import { useLayout } from "./store/layout";
import { useNavigation } from "./store/navigation";

describe("App", () => {
  beforeEach(async () => {
    await i18n.changeLanguage("fr");
    useNavigation.setState({ screen: "import" });
    useLayout.setState({ visible: { left: true, right: true, filmstrip: true }, collapsed: {} });
  });

  it("affiche les étapes du flux puis les vues transverses", () => {
    render(<App />);
    const modules = screen.getByRole("navigation", { name: "Modules" });
    const labels = Array.from(modules.querySelectorAll("button")).map((b) => b.textContent);
    expect(labels).toEqual([
      "Importer",
      "Trier",
      "Retoucher",
      "Livrer",
      "Bibliothèque",
      "Carte",
      "Matériel",
    ]);
  });

  it("change de module au clic", () => {
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "Trier" }));
    expect(screen.getByRole("button", { name: "Trier" })).toHaveAttribute("aria-current", "page");
    expect(screen.getByRole("heading", { name: "Aucune photo à trier" })).toBeInTheDocument();
  });

  it("signale les modules des versions suivantes", () => {
    render(<App />);
    // Les modules de la version en cours ne sont pas marqués.
    expect(screen.getByRole("button", { name: "Retoucher" })).not.toHaveAttribute("title");
    expect(screen.getByRole("button", { name: "Carte" })).not.toHaveAttribute("title");
  });

  it("replie une section de panneau", () => {
    render(<App />);
    const header = screen.getByRole("button", { name: "Destination" });
    expect(header).toHaveAttribute("aria-expanded", "true");
    fireEvent.click(header);
    expect(header).toHaveAttribute("aria-expanded", "false");
  });

  it("masque le panneau de gauche avec F7, comme Lightroom", () => {
    render(<App />);
    expect(screen.getByRole("complementary", { name: "Panneau de gauche" })).toBeInTheDocument();
    handleMenuShortcut(new KeyboardEvent("keydown", { key: "F7" }));
    expect(useLayout.getState().visible.left).toBe(false);
  });

  it("bascule en anglais", async () => {
    await i18n.changeLanguage("en");
    render(<App />);
    const modules = screen.getByRole("navigation", { name: "Modules" });
    expect(within(modules).getByRole("button", { name: "Library" })).toBeInTheDocument();
  });
});

describe("Accueil", () => {
  it("n'est plus un module de la fenêtre principale", () => {
    render(<App />);
    const modules = screen.getByRole("navigation", { name: "Modules" });
    expect(within(modules).queryByRole("button", { name: "Accueil" })).toBeNull();
  });
});
