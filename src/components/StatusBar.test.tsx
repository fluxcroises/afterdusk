import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

const { default: i18n } = await import("../i18n");
await i18n.changeLanguage("fr");
const { useRescan } = await import("../store/rescan");
const { StatusBar } = await import("./StatusBar");

describe("barre d'état", () => {
  it("affiche la progression de la réanalyse, sans boucle de rendu", () => {
    // Réanalyse lancée, progression pas encore reçue : la barre doit s'afficher.
    useRescan.setState({ running: true, progress: null });
    const { rerender } = render(<StatusBar />);
    expect(screen.getByText("Réanalyse : 0 / 0")).toBeInTheDocument();
    useRescan.setState({ progress: { done: 3, total: 8 } });
    rerender(<StatusBar />);
    expect(screen.getByText("Réanalyse : 3 / 8")).toBeInTheDocument();
    useRescan.setState({ running: false, progress: null });
  });
});
