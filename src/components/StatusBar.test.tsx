import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

const { default: i18n } = await import("../i18n");
await i18n.changeLanguage("fr");
const { useRescan } = await import("../store/rescan");
const { useWarnings } = await import("../store/warnings");
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

  it("signale les XMP non écrits et les aperçus manqués, en texte", () => {
    const warnings = useWarnings.getState();
    warnings.onWarning({ kind: "xmp", count: 3, detail: "accès refusé" });
    warnings.onWarning({ kind: "preview", count: 1, detail: "" });
    warnings.onWarning({ kind: "preview", count: 1, detail: "" });
    const { rerender } = render(<StatusBar />);
    expect(
      screen.getByText("XMP non écrits pour 3 photos, notes gardées dans la base"),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByText("2 aperçus n'ont pas pu être créés"));
    warnings.onWarning({ kind: "xmpRecovered", count: 0, detail: "" });
    rerender(<StatusBar />);
    expect(screen.queryByText(/aperçus n'ont pas pu/)).not.toBeInTheDocument();
    expect(screen.queryByText(/XMP non écrits/)).not.toBeInTheDocument();
  });
});
