import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: vi.fn(),
}));

const { invoke } = await import("@tauri-apps/api/core");
const {
  HISTOGRAM_BINS,
  parseFrame,
  positionFromTemperature,
  SAVE_DELAY_MS,
  temperatureFromPosition,
  useDevelop,
  whitePoint,
} = await import("./develop");

/** Rendu factice de 2 × 1 pixels, comme le renvoie `develop_render`. */
function frame(gpu: boolean) {
  const buffer = new ArrayBuffer(16 + HISTOGRAM_BINS * 4 + 8);
  const view = new DataView(buffer);
  view.setUint32(0, 2, true);
  view.setUint32(4, 1, true);
  view.setUint32(8, 1500, true);
  view.setUint32(12, gpu ? 1 : 0, true);
  const histogram = new Uint32Array(buffer, 16, HISTOGRAM_BINS);
  histogram[255] = 2;
  histogram[768 + 40] = 2;
  new Uint8Array(buffer, 16 + HISTOGRAM_BINS * 4).set([10, 20, 30, 255, 40, 50, 60, 255]);
  return buffer;
}

const presets = [
  { name: "daylight", temperature: 5500, tint: 10 },
  { name: "tungsten", temperature: 2850, tint: 0 },
];

const mocked = vi.mocked(invoke);

beforeEach(() => {
  vi.useFakeTimers();
  mocked.mockReset();
  mocked.mockImplementation(async (cmd: string) => {
    if (cmd === "develop_open")
      return {
        mediaId: 7,
        width: 2,
        height: 1,
        settings: { version: 1, process: 1 },
        asShot: { temperature: 4550, tint: -12 },
        presets,
        adapter: "Carte de test",
        decodeMs: 210,
      };
    if (cmd === "develop_render") return frame(true);
    if (cmd === "develop_pick_white") return { mode: "custom", temperature: 3900, tint: 6 };
    if (cmd === "develop_auto_light")
      return {
        version: 1,
        process: 1,
        light: { exposure: 0.45, shadows: 22, blacks: -12 },
      };
    if (cmd === "develop_auto_white") return { mode: "custom", temperature: 5100, tint: 3 };
    return null;
  });
});

afterEach(async () => {
  await useDevelop.getState().close();
  useDevelop.setState({ clipping: { highlights: false, shadows: false } });
  vi.useRealTimers();
});

const calls = (cmd: string) => mocked.mock.calls.filter(([c]) => c === cmd);
const saves = () => calls("develop_save").map(([, args]) => args);

describe("Retoucher", () => {
  it("lit l'en-tête, l'histogramme et les pixels du rendu", () => {
    const f = parseFrame(frame(false));
    expect(f).toMatchObject({ width: 2, height: 1, renderMs: 1.5, gpu: false });
    expect(f.histogram[255]).toBe(2);
    expect(f.histogram[768 + 40]).toBe(2);
    expect(Array.from(f.pixels.slice(4, 7))).toEqual([40, 50, 60]);
  });

  it("ouvre la photo et affiche le rendu neutre", async () => {
    await useDevelop.getState().open(7);
    const s = useDevelop.getState();
    expect(s.settings).toEqual({ version: 1, process: 1 });
    expect(s.asShot).toEqual({ temperature: 4550, tint: -12 });
    expect(s.frame?.gpu).toBe(true);
    expect(calls("develop_render")[0]?.[1]).toEqual({
      settings: { version: 1, process: 1 },
      clipping: { highlights: false, shadows: false },
    });
  });

  it("rend à chaque geste et n'enregistre qu'une fois, après le dernier", async () => {
    await useDevelop.getState().open(7);
    useDevelop.getState().setLight("exposure", 0.3);
    useDevelop.getState().setLight("exposure", 0.7);
    useDevelop.getState().setLight("exposure", 1.234);
    await vi.waitFor(() => expect(useDevelop.getState().settings?.light?.exposure).toBe(1.23));
    expect(saves()).toHaveLength(0);
    await vi.advanceTimersByTimeAsync(SAVE_DELAY_MS + 10);
    expect(saves()).toEqual([
      {
        mediaId: 7,
        settings: { version: 1, process: 1, light: { exposure: 1.23 } },
        label: "light.exposure",
      },
    ]);
    // Les gestes rapprochés ne lancent pas un rendu chacun.
    expect(calls("develop_render").length).toBeLessThanOrEqual(3);
  });

  it("revenir à zéro retire le réglage du JSON creux", async () => {
    await useDevelop.getState().open(7);
    useDevelop.getState().setLight("contrast", 30);
    useDevelop.getState().setLight("contrast", 0);
    expect(useDevelop.getState().settings).toEqual({ version: 1, process: 1, light: {} });
  });

  it("chaque curseur a sa propre étape d'historique", async () => {
    await useDevelop.getState().open(7);
    useDevelop.getState().setLight("shadows", 40);
    useDevelop.getState().setColor("vibrance", 25.4);
    await vi.advanceTimersByTimeAsync(SAVE_DELAY_MS + 10);
    expect(saves()).toEqual([
      {
        mediaId: 7,
        settings: { version: 1, process: 1, light: { shadows: 40 } },
        label: "light.shadows",
      },
      {
        mediaId: 7,
        settings: { version: 1, process: 1, light: { shadows: 40 }, color: { vibrance: 25 } },
        label: "color.vibrance",
      },
    ]);
  });

  it("la température part de la balance du boîtier et garde sa teinte", async () => {
    await useDevelop.getState().open(7);
    useDevelop.getState().setTemperature(6000);
    expect(useDevelop.getState().settings?.color?.whiteBalance).toEqual({
      mode: "custom",
      temperature: 6000,
      tint: -12,
    });
    useDevelop.getState().setWhiteBalance({ mode: "preset", preset: "tungsten" });
    const { settings, asShot } = useDevelop.getState();
    expect(asShot).not.toBeNull();
    if (!asShot) return;
    expect(whitePoint(settings?.color?.whiteBalance, asShot, presets)).toEqual({
      name: "tungsten",
      temperature: 2850,
      tint: 0,
    });
    useDevelop.getState().setWhiteBalance({ mode: "asShot" });
    expect(useDevelop.getState().settings?.color).toEqual({});
  });

  it("la pipette règle la balance depuis le point cliqué", async () => {
    await useDevelop.getState().open(7);
    useDevelop.getState().setPicking(true);
    await useDevelop.getState().pickWhite(120.4, 33.6);
    expect(calls("develop_pick_white")[0]?.[1]).toEqual({ x: 120, y: 34 });
    expect(useDevelop.getState().picking).toBe(false);
    expect(useDevelop.getState().settings?.color?.whiteBalance).toEqual({
      mode: "custom",
      temperature: 3900,
      tint: 6,
    });
  });

  it("les avertissements d'écrêtage relancent le rendu sans rien enregistrer", async () => {
    await useDevelop.getState().open(7);
    useDevelop.getState().toggleClipping("highlights");
    await vi.waitFor(() => expect(calls("develop_render")).toHaveLength(2));
    expect(calls("develop_render")[1]?.[1]).toMatchObject({
      clipping: { highlights: true, shadows: false },
    });
    await vi.advanceTimersByTimeAsync(SAVE_DELAY_MS + 10);
    expect(saves()).toHaveLength(0);
  });

  it("Auto règle la lumière en une seule étape d'historique", async () => {
    await useDevelop.getState().open(7);
    useDevelop.getState().setLight("brightness", 15);
    await useDevelop.getState().autoLight();
    expect(calls("develop_auto_light")[0]?.[1]).toEqual({
      settings: { version: 1, process: 1, light: { brightness: 15 } },
      key: null,
    });
    expect(useDevelop.getState().settings?.light).toEqual({
      exposure: 0.45,
      shadows: 22,
      blacks: -12,
    });
    expect(useDevelop.getState().autoBusy).toBe(false);
    await vi.advanceTimersByTimeAsync(SAVE_DELAY_MS + 10);
    expect(saves().map((a) => (a as { label: string }).label)).toEqual([
      "light.brightness",
      "light.auto",
    ]);
  });

  it("Maj + double-clic : l'auto d'un seul curseur", async () => {
    await useDevelop.getState().open(7);
    await useDevelop.getState().autoLight("shadows");
    expect(calls("develop_auto_light")[0]?.[1]).toMatchObject({ key: "shadows" });
    await vi.advanceTimersByTimeAsync(SAVE_DELAY_MS + 10);
    expect(saves()[0]).toMatchObject({ label: "light.shadows.auto" });
  });

  it("balance des blancs automatique", async () => {
    await useDevelop.getState().open(7);
    await useDevelop.getState().autoWhite();
    expect(useDevelop.getState().settings?.color?.whiteBalance).toEqual({
      mode: "custom",
      temperature: 5100,
      tint: 3,
    });
    await vi.advanceTimersByTimeAsync(SAVE_DELAY_MS + 10);
    expect(saves()[0]).toMatchObject({ label: "color.whiteBalance.auto" });
  });

  it("un auto arrivé après un changement de photo est ignoré", async () => {
    await useDevelop.getState().open(7);
    let release: (v: unknown) => void = () => undefined;
    mocked.mockImplementationOnce(() => new Promise((r) => (release = r)));
    const pending = useDevelop.getState().autoLight();
    useDevelop.setState({ mediaId: 8 });
    release({ version: 1, process: 1, light: { exposure: 2 } });
    await pending;
    expect(useDevelop.getState().settings?.light).toBeUndefined();
  });

  it("curseur de température régulier en mireds", () => {
    expect(temperatureFromPosition(0)).toBe(2000);
    expect(temperatureFromPosition(1)).toBe(50000);
    for (const k of [2850, 4000, 5500, 6500, 10000]) {
      expect(Math.abs(temperatureFromPosition(positionFromTemperature(k)) - k)).toBeLessThanOrEqual(
        25,
      );
    }
    // Le milieu du curseur tombe dans les températures courantes, pas à 26 000 K.
    expect(temperatureFromPosition(0.5)).toBeLessThan(4000);
  });

  it("changer de photo enregistre le réglage en attente", async () => {
    await useDevelop.getState().open(7);
    useDevelop.getState().setLight("exposure", -0.5);
    await useDevelop.getState().open(8);
    expect(saves()[0]).toMatchObject({ mediaId: 7, settings: { light: { exposure: -0.5 } } });
  });

  it("une photo non prise en charge donne une erreur traduisible", async () => {
    mocked.mockImplementation(async (cmd: string) => {
      if (cmd === "develop_open") throw { code: "developUnsupported", detail: "IMG_0001.JPG" };
      return null;
    });
    await useDevelop.getState().open(9);
    expect(useDevelop.getState().error?.code).toBe("developUnsupported");
    expect(useDevelop.getState().loading).toBe(false);
  });
});
