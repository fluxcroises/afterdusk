import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import i18n from "../i18n";
import { SecondaryView } from "./SecondaryApp";
import { secondaryState } from "./sync";

const photo = (id: number) => ({
  id,
  albumId: 1,
  fileName: `IMG_${id}.CR3`,
  capturedAt: null,
  rating: 0,
  flag: 0,
  color: null,
  width: 6000,
  height: 4000,
  orientation: 1,
  hasPreview: false,
});

const media = [1, 2, 3, 4, 5, 6, 7, 8].map(photo);

describe("second écran", () => {
  beforeEach(async () => {
    await i18n.changeLanguage("fr");
    window.localStorage.clear();
  });

  it("envoie la photo active, la comparaison et les voisines, pas toute la bibliothèque", () => {
    const state = secondaryState({ media, selectedId: 5, selectedIds: [2, 5], versions: { 5: 3 } });
    expect(state.activeId).toBe(5);
    expect(state.compare).toEqual([2, 5]);
    expect(state.photos.map((m) => m.id)).toEqual([2, 3, 4, 5, 6, 7]);
    expect(state.versions[5]).toBe(3);
  });

  it("montre la photo active, ou la comparaison au choix", () => {
    const state = secondaryState({ media, selectedId: 3, selectedIds: [3, 4, 5], versions: {} });
    const { container } = render(<SecondaryView state={state} />);
    expect(container.querySelectorAll(".compare-pane")).toHaveLength(1);
    fireEvent.click(screen.getByRole("button", { name: "Comparer" }));
    expect(container.querySelectorAll(".compare-pane")).toHaveLength(3);
    // Le choix est mémorisé pour la prochaine ouverture.
    expect(window.localStorage.getItem("afterdusk.secondaryMode")).toBe("compare");
  });

  it("invite à choisir une photo tant que la fenêtre principale n'en a pas", () => {
    render(<SecondaryView state={null} />);
    expect(screen.getByText(/Sélectionnez une photo/)).toBeInTheDocument();
  });
});
