import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn<(cmd: string, args?: Record<string, unknown>) => Promise<unknown>>();

vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  isTauri: () => true,
  convertFileSrc: (_path: string, protocol: string) => `${protocol}://localhost/`,
  invoke: (cmd: string, args?: Record<string, unknown>) => invoke(cmd, args),
}));

const { default: i18n } = await import("../i18n");
await i18n.changeLanguage("fr");
const { useLibrary } = await import("../store/library");
const { useTemplates } = await import("../store/templates");
const { TemplatesView } = await import("./DeliveryTemplates");

const templates = [
  { id: "instagram-portrait", group: "social", width: 1080, height: 1350 },
  { id: "print-10x15", group: "print", width: 1800, height: 1200 },
];

const framings = [
  {
    template: "instagram-portrait",
    rect: [0.2, 0, 0.73, 1],
    custom: false,
    width: 1080,
    height: 1350,
    sharpEnough: true,
  },
  {
    template: "print-10x15",
    rect: [0, 0, 1, 1],
    custom: true,
    width: 1800,
    height: 1200,
    sharpEnough: false,
  },
];

async function show() {
  useLibrary.setState({ selectedId: 7, selectedIds: [7], versions: {} });
  render(<TemplatesView />);
  await act(async () => {});
}

describe("modèles de livraison", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation((cmd) =>
      Promise.resolve(
        cmd === "delivery_templates" ? templates : cmd === "delivery_framings" ? framings : null,
      ),
    );
    useTemplates.setState({ templates: [], framings: [], chosen: [], editing: null, item: null });
  });

  it("range les modèles par usage, avec leur taille et leurs alertes", async () => {
    await show();
    expect(invoke).toHaveBeenCalledWith("delivery_framings", { item: 7 });
    expect(screen.getByRole("heading", { name: "Réseaux sociaux" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Tirages" })).toBeInTheDocument();
    expect(screen.getByText("1080 × 1350 px")).toBeInTheDocument();
    expect(screen.getByText("Définition juste")).toBeInTheDocument();
    expect(screen.getByText("Cadré à la main")).toBeInTheDocument();
  });

  it("un clic coche le modèle, un second le décoche", async () => {
    await show();
    const card = screen.getByRole("button", { name: /^Instagram 4:5/ });
    fireEvent.click(card, { detail: 1 });
    expect(useTemplates.getState().chosen).toEqual(["instagram-portrait"]);
    expect(card).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(card, { detail: 1 });
    expect(useTemplates.getState().chosen).toEqual([]);
  });

  it("le double-clic coche et ouvre le cadrage, enregistré au rapport du modèle", async () => {
    await show();
    fireEvent.doubleClick(screen.getByRole("button", { name: /^Instagram 4:5/ }));
    expect(useTemplates.getState().chosen).toEqual(["instagram-portrait"]);
    expect(screen.getByRole("dialog", { name: "Cadrage : Instagram 4:5" })).toBeInTheDocument();
    // Le cadre ne s'affiche qu'une fois l'image chargée.
    const img = screen.getByRole("dialog").querySelector("img");
    if (!img) throw new Error("image absente");
    Object.defineProperty(img, "naturalWidth", { value: 6000 });
    Object.defineProperty(img, "naturalHeight", { value: 4000 });
    fireEvent.load(img);
    const frame = screen.getByRole("group", { name: /Cadre/ });
    fireEvent.keyDown(frame, { key: "ArrowRight", shiftKey: true });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Enregistrer" }));
    });
    const set = invoke.mock.calls.find(([cmd]) => cmd === "delivery_framing_set");
    expect(set?.[1]?.template).toBe("instagram-portrait");
    const rect = set?.[1]?.rect as number[];
    expect(rect[0]).toBeCloseTo(0.3);
    expect(rect[2]).toBeCloseTo(0.83);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("le cadrage automatique se rétablit pour un cadre posé à la main", async () => {
    await show();
    fireEvent.click(screen.getByRole("button", { name: "Cadrer « Tirage 10 × 15 »" }));
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Cadrage automatique" }));
    });
    expect(invoke).toHaveBeenCalledWith("delivery_framing_reset", {
      item: 7,
      template: "print-10x15",
    });
  });
});
