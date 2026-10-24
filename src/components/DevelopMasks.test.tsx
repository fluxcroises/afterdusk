import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: () => Promise.resolve(null),
}));

const { default: i18n } = await import("../i18n");
await i18n.changeLanguage("fr");
const { useDevelop } = await import("../store/develop");
const { useLayout } = await import("../store/layout");
const { MaskOverlay, MasksSection } = await import("./DevelopMasks");

const linear = { shape: { kind: "linear" as const, from: [0.5, 0], to: [0.5, 0.5] } };
const radial = {
  shape: { kind: "radial" as const, center: [0.5, 0.5], radius: [0.25, 0.25] },
  adjust: { exposure: 0.5 },
};

beforeAll(() => {
  // jsdom : ni capture du pointeur, ni contexte de dessin.
  HTMLElement.prototype.setPointerCapture = () => undefined;
  HTMLCanvasElement.prototype.getContext = () => null;
});

function setup(masks: object[], selectedMask: number | null = null) {
  const spies = {
    armMask: vi.fn(),
    addMask: vi.fn(() => Promise.resolve()),
    setMaskShape: vi.fn(() => Promise.resolve()),
    selectMask: vi.fn(),
    setMaskAdjust: vi.fn(),
    toggleMask: vi.fn(() => Promise.resolve()),
  };
  useLayout.setState({ collapsed: { "develop.masks": false } });
  useDevelop.setState({
    settings: { version: 2, process: 1, masks } as never,
    sourceSize: { width: 400, height: 200 },
    selectedMask,
    maskDraft: null,
    showMask: false,
    zoom: null,
    ...spies,
  });
  return spies;
}

const box = { left: 0, top: 0, width: 400, height: 200 };
const view = { x0: 0, y0: 0, x1: 1, y1: 1 };

describe("outil Masques", () => {
  afterEach(() => useDevelop.setState({ settings: null, selectedMask: null, maskDraft: null }));

  it("sans masque : les deux dégradés et l'invitation à tracer", () => {
    const spies = setup([]);
    render(<MasksSection />);
    expect(screen.getByText("Choisir un dégradé, puis glisser sur la photo.")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Radial" }));
    expect(spies.armMask).toHaveBeenCalledWith("radial");
    expect(screen.queryAllByRole("slider")).toHaveLength(0);
  });

  it("liste numérotée par forme, réglages du masque choisi", () => {
    const spies = setup([linear, radial, linear], 1);
    render(<MasksSection />);
    for (const name of ["Linéaire 1", "Radial 1", "Linéaire 2"]) {
      expect(screen.getByRole("button", { name })).toBeInTheDocument();
    }
    // Sept réglages et l'adoucissement du radial.
    expect(screen.getAllByRole("slider")).toHaveLength(8);
    expect(screen.getByText("+0,50 IL")).toBeInTheDocument();
    const saturation = screen.getByText("Saturation").closest("label")?.querySelector("input");
    fireEvent.change(saturation as HTMLInputElement, { target: { value: "30" } });
    expect(spies.setMaskAdjust).toHaveBeenCalledWith(1, "saturation", 30);
    fireEvent.click(screen.getAllByRole("button", { name: "Masquer" })[0] as HTMLElement);
    expect(spies.toggleMask).toHaveBeenCalledWith(0, "hidden");
  });

  it("outil armé : un glissé trace le dégradé linéaire", () => {
    const spies = setup([]);
    useDevelop.setState({ maskDraft: "linear" });
    render(<MaskOverlay box={box} view={view} />);
    const layer = screen.getByRole("group", { name: "Masques" });
    fireEvent.pointerDown(layer, { button: 0, clientX: 200, clientY: 0 });
    fireEvent.pointerMove(layer, { clientX: 200, clientY: 100 });
    fireEvent.pointerUp(layer, { clientX: 200, clientY: 100 });
    expect(spies.addMask).toHaveBeenCalledWith({ kind: "linear", from: [0.5, 0], to: [0.5, 0.5] });
  });

  it("poignée du radial choisi : la taille suit le pointeur, Maj donne un cercle", () => {
    const spies = setup([radial], 0);
    render(<MaskOverlay box={box} view={view} />);
    const layer = screen.getByRole("group", { name: "Masques" });
    // Poignée « u+ » : 100 pixels à droite du centre (0,25 × 400).
    fireEvent.pointerDown(layer, { button: 0, clientX: 300, clientY: 100 });
    fireEvent.pointerMove(layer, { clientX: 340, clientY: 100, shiftKey: true });
    fireEvent.pointerUp(layer, { clientX: 340, clientY: 100 });
    expect(spies.setMaskShape).toHaveBeenCalledWith(0, {
      ...radial.shape,
      radius: [0.35, 0.7],
    });
  });

  it("clic sur la pastille d'un autre masque : il est choisi", () => {
    const ailleurs = { shape: { ...radial.shape, center: [0.25, 0.5] } };
    const spies = setup([linear, ailleurs], 0);
    render(<MaskOverlay box={box} view={view} />);
    const layer = screen.getByRole("group", { name: "Masques" });
    fireEvent.pointerDown(layer, { button: 0, clientX: 101, clientY: 101 });
    fireEvent.pointerUp(layer, { clientX: 101, clientY: 101 });
    expect(spies.selectMask).toHaveBeenCalledWith(1);
  });
});
