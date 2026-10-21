import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { useDialog } from "../store/dialog";
import { confirmWarning } from "../ui/confirm";
import { AppDialog } from "./AppDialog";

describe("boîtes de dialogue d'Afterdusk", () => {
  beforeEach(() => useDialog.setState({ current: null, queue: [] }));

  it("confirmation destructive : Annuler en avant, Échap annule", async () => {
    render(<AppDialog />);
    let answer: Promise<boolean> = Promise.resolve(true);
    act(() => {
      answer = confirmWarning("Supprimer « Nettoyage » ?", {
        title: "Entretien",
        okLabel: "Supprimer",
        cancelLabel: "Annuler",
      });
    });
    const dialog = screen.getByRole("alertdialog", { name: "Entretien" });
    expect(dialog).toHaveTextContent("Supprimer « Nettoyage » ?");
    expect(screen.getByRole("button", { name: "Annuler" })).toHaveFocus();
    act(() => {
      fireEvent.keyDown(window, { key: "Escape" });
    });
    await expect(answer).resolves.toBe(false);
    expect(screen.queryByRole("alertdialog")).toBeNull();
  });

  it("validée, puis une seconde demande attend son tour", async () => {
    render(<AppDialog />);
    let first: Promise<boolean> = Promise.resolve(false);
    let second: Promise<boolean> = Promise.resolve(false);
    act(() => {
      first = confirmWarning("A ?", { title: "Un", okLabel: "Oui", cancelLabel: "Non" });
      second = useDialog
        .getState()
        .show({ kind: "message", title: "Deux", text: "Fini.", okLabel: "OK" });
    });
    fireEvent.click(screen.getByRole("button", { name: "Oui" }));
    await expect(first).resolves.toBe(true);
    // Un message : une seule réponse possible, OK.
    const message = await screen.findByRole("dialog", { name: "Deux" });
    expect(message).toHaveTextContent("Fini.");
    expect(screen.queryByRole("button", { name: "Non" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "OK" }));
    await expect(second).resolves.toBe(true);
  });
});
