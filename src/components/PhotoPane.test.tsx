import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";
import type { Zoom } from "./PhotoPane";

vi.mock("../ui/preload", () => ({
  displaySrc: () => "preview.jpg",
  preload: () => Promise.resolve(),
}));
vi.mock("../api/previews", () => ({ fullUrl: () => "full.jpg" }));

const { default: i18n } = await import("../i18n");
await i18n.changeLanguage("fr");
const { PhotoPane } = await import("./PhotoPane");

// Mise en page simulée : vue de 1000 × 600, aperçu de 2048 × 1365 ajusté
// dedans (900 × 600, à 50 px du bord gauche).
const VIEW = { width: 1000, height: 600 };
const restore: (() => void)[] = [];
function stub<T extends object>(proto: T, key: string, descriptor: PropertyDescriptor) {
  const previous = Object.getOwnPropertyDescriptor(proto, key);
  Object.defineProperty(proto, key, { configurable: true, ...descriptor });
  restore.push(() =>
    previous ? Object.defineProperty(proto, key, previous) : Reflect.deleteProperty(proto, key),
  );
}

beforeAll(() => {
  stub(HTMLElement.prototype, "clientWidth", { get: () => VIEW.width });
  stub(HTMLElement.prototype, "clientHeight", { get: () => VIEW.height });
  stub(Element.prototype, "getBoundingClientRect", {
    value: () => ({ left: 0, top: 0, width: VIEW.width, height: VIEW.height }),
  });
  stub(HTMLImageElement.prototype, "naturalWidth", {
    get(this: HTMLImageElement) {
      return this.className === "loupe-full" ? 7728 : 2048;
    },
  });
  stub(HTMLImageElement.prototype, "naturalHeight", {
    get(this: HTMLImageElement) {
      return this.className === "loupe-full" ? 5152 : 1365;
    },
  });
  stub(Element.prototype, "setPointerCapture", { value: () => undefined });
  const Previous = globalThis.ResizeObserver;
  globalThis.ResizeObserver = class {
    constructor(private readonly callback: ResizeObserverCallback) {}
    observe() {
      this.callback([], this as unknown as ResizeObserver);
    }
    unobserve() {}
    disconnect() {}
  };
  restore.push(() => (globalThis.ResizeObserver = Previous));
});
afterAll(() => restore.forEach((r) => r()));

const photo = (width: number | null, height: number | null) => ({
  id: 1,
  albumId: 1,
  fileName: "DSCF1667.JPG",
  capturedAt: null,
  rating: 0,
  flag: 0,
  color: null,
  width,
  height,
  orientation: 1,
  hasPreview: true,
  trashedAt: null,
  stackSize: 1,
  companionOf: null,
});

/** Visionneuse dont le zoom est tenu par le test, comme par le store. */
async function mount(width: number | null = 7728, height: number | null = 5152) {
  let zoom: Zoom = null;
  const onZoom = vi.fn((z: Zoom) => {
    zoom = z;
    view.rerender(
      <PhotoPane media={photo(width, height)} version={0} zoom={zoom} onZoom={onZoom} />,
    );
  });
  const view = render(
    <PhotoPane media={photo(width, height)} version={0} zoom={zoom} onZoom={onZoom} />,
  );
  await act(async () => {});
  return { onZoom, zoom: () => zoom, loupe: view.container.querySelector(".loupe") as HTMLElement };
}

describe("visionneuse : zoom", () => {
  it("la molette zoome progressivement, puis revient à l'ajusté", async () => {
    const { zoom, loupe } = await mount();
    // Ajusté : 1000 / 7728 ≈ 0,129 et 600 / 5152 ≈ 0,116.
    fireEvent.wheel(loupe, { deltaY: -100, clientX: 500, clientY: 300 });
    expect(zoom()?.scale).toBeCloseTo((600 / 5152) * 1.25, 4);
    expect(zoom()?.fx).toBeCloseTo(0.5, 2);
    fireEvent.wheel(loupe, { deltaY: -100, clientX: 500, clientY: 300 });
    expect(zoom()?.scale).toBeCloseTo((600 / 5152) * 1.25 ** 2, 4);
    expect(screen.getByText("18 %")).toBeInTheDocument();
    fireEvent.wheel(loupe, { deltaY: 100, clientX: 500, clientY: 300 });
    fireEvent.wheel(loupe, { deltaY: 100, clientX: 500, clientY: 300 });
    expect(zoom()).toBeNull();
  });

  it("un rectangle tracé sur la photo remplit la vue", async () => {
    const { zoom, loupe } = await mount();
    fireEvent.pointerDown(loupe, { button: 0, clientX: 400, clientY: 200, pointerId: 1 });
    fireEvent.pointerMove(loupe, { clientX: 600, clientY: 400, pointerId: 1 });
    expect(loupe.querySelector(".loupe-marquee")).not.toBeNull();
    fireEvent.pointerUp(loupe, { clientX: 600, clientY: 400, pointerId: 1 });
    expect(loupe.querySelector(".loupe-marquee")).toBeNull();
    // Rectangle de 0,222 × 0,333 de la photo, centré : la hauteur limite.
    expect(zoom()?.fx).toBeCloseTo(0.5, 3);
    expect(zoom()?.fy).toBeCloseTo(0.5, 3);
    expect(zoom()?.scale).toBeCloseTo(600 / ((1 / 3) * 5152), 3);
  });

  it("un simple clic zoome toujours à 100 %", async () => {
    const { zoom } = await mount();
    fireEvent.click(screen.getByAltText("DSCF1667.JPG"), { clientX: 500, clientY: 300 });
    expect(zoom()).toEqual({ fx: 0.5, fy: 0.5 });
  });

  it("photo sans dimensions en base : la pleine taille les donne, pas d'écran noir", async () => {
    const { loupe } = await mount(null, null);
    fireEvent.click(screen.getByAltText("DSCF1667.JPG"), { clientX: 500, clientY: 300 });
    const full = loupe.querySelector(".loupe-full") as HTMLImageElement;
    // Chargée d'abord sans être montrée…
    expect(full).not.toBeNull();
    expect((loupe.querySelector(".loupe-zoomed") as HTMLElement).style.visibility).toBe("hidden");
    fireEvent.load(full);
    // … puis montrée à sa taille réelle.
    const zoomed = loupe.querySelector(".loupe-zoomed") as HTMLElement;
    expect(zoomed.style.visibility).toBe("");
    expect(zoomed.style.width).toBe("7728px");
    expect(screen.getByText("100 %")).toBeInTheDocument();
  });
});
