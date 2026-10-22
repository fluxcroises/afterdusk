import { act, fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn<(cmd: string, args?: Record<string, unknown>) => Promise<unknown>>();

vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  isTauri: () => true,
  convertFileSrc: (_path: string, protocol: string) => `${protocol}://localhost/`,
  invoke: (cmd: string, args?: Record<string, unknown>) => invoke(cmd, args),
}));

// jsdom n'a pas de canvas : la page Konva devient de simples éléments.
vi.mock("react-konva", () => {
  const box = ({ children }: { children?: ReactNode }) => <div>{children}</div>;
  return {
    Stage: box,
    Layer: box,
    Group: box,
    Rect: () => null,
    Image: () => null,
    Text: ({ text }: { text: string }) => <span data-testid="page-text">{text}</span>,
  };
});

const { default: i18n } = await import("../i18n");
await i18n.changeLanguage("fr");
const { useDesigns } = await import("../store/designs");
const { useDialog } = await import("../store/dialog");
const { DesignEditor } = await import("./DesignEditor");

async function open() {
  await act(async () => {
    await useDesigns.getState().create("facebook-post", 1200, 630, [7, 8], "Faire-part");
  });
  render(<DesignEditor />);
}

describe("éditeur de livraison", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation((cmd) =>
      Promise.resolve(
        cmd === "design_focus"
          ? [0.4, 0.3]
          : cmd === "design_save"
            ? 12
            : cmd === "design_list"
              ? []
              : cmd === "design_export"
                ? "D:/Livraison/Faire-part.jpg"
                : null,
      ),
    );
    useDesigns.setState({ draft: null, dirty: false, selected: null, exported: null });
  });

  it("part des photos choisies, centrées sur leur visage", async () => {
    await open();
    const d = useDesigns.getState().draft?.document;
    expect(d?.photos).toEqual([
      { item: 7, focus: [0.4, 0.3] },
      { item: 8, focus: [0.4, 0.3] },
    ]);
    expect(d?.margin).toBe(3);
    expect(
      screen.getByRole("dialog", { name: /Composition : Facebook et LinkedIn/ }),
    ).toBeVisible();
    expect(screen.getByText("2 photos")).toBeInTheDocument();
  });

  it("ajoute un texte, le modifie, puis exporte après l'avoir enregistré", async () => {
    await open();
    fireEvent.click(screen.getByRole("button", { name: "Ajouter un texte" }));
    expect(screen.getByTestId("page-text")).toHaveTextContent("Votre texte");
    fireEvent.change(screen.getByRole("textbox", { name: "Texte" }), {
      target: { value: "Léa & Hugo" },
    });
    fireEvent.click(screen.getByRole("radio", { name: "Droite" }));
    const text = useDesigns.getState().draft?.document.texts[0];
    expect(text?.text).toBe("Léa & Hugo");
    expect(text?.align).toBe("right");
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Exporter" }));
    });
    const calls = invoke.mock.calls.map(([cmd]) => cmd);
    expect(calls.indexOf("design_save")).toBeLessThan(calls.indexOf("design_export"));
    expect(invoke).toHaveBeenCalledWith("design_export", { id: 12 });
    expect(screen.getByText("Exportée : D:/Livraison/Faire-part.jpg")).toBeInTheDocument();
  });

  it("demande avant de fermer une composition non enregistrée", async () => {
    await open();
    fireEvent.click(screen.getByRole("button", { name: "Fermer l'éditeur" }));
    await act(async () => {});
    expect(useDialog.getState().current?.title).toBe("Fermer sans enregistrer ?");
    await act(async () => useDialog.getState().answer(false));
    expect(useDesigns.getState().draft).not.toBeNull();
  });
});
