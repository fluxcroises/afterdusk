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

/** Historique tenu par le « cœur » simulé : étapes et position courante. */
let steps: { position: number; label: string; settings: object; createdAt: string }[] = [];
let current = 0;
let coreSettings: object = { version: 1, process: 1 };
let snapshots: { name: string; settings: object; createdAt: string }[] = [];
let versions: { name: string; createdAt: string }[] = [];

beforeEach(() => {
  vi.useFakeTimers();
  mocked.mockReset();
  steps = [];
  current = 0;
  coreSettings = { version: 1, process: 1 };
  snapshots = [];
  versions = [];
  mocked.mockImplementation(async (cmd: string, args?: unknown) => {
    const a = (args ?? {}) as Record<string, unknown>;
    if (cmd === "develop_save") {
      steps = steps.filter((st) => st.position <= current);
      current += 1;
      steps.push({
        position: current,
        label: a.label as string,
        settings: a.settings as object,
        createdAt: "",
      });
      coreSettings = a.settings as object;
      return a.settings;
    }
    if (cmd === "develop_state")
      return { settings: coreSettings, history: { steps, current }, snapshots, versions };
    if (cmd === "develop_snapshot_create") {
      snapshots.push({ name: a.name as string, settings: coreSettings, createdAt: "" });
      return null;
    }
    if (cmd === "develop_version_create") {
      versions.push({ name: a.name as string, createdAt: "" });
      return null;
    }
    if (cmd === "develop_history") return { steps, current };
    if (cmd === "develop_go_to") {
      current = a.position as number;
      coreSettings =
        current === 0
          ? { version: 1, process: 1 }
          : (steps.find((st) => st.position === current)?.settings ?? coreSettings);
      return coreSettings;
    }
    if (cmd === "develop_open")
      return {
        mediaId: 7,
        width: 2,
        height: 1,
        sourceWidth: 2,
        sourceHeight: 1,
        lens: "Sony FE 16-35mm f/2.8 GM",
        settings: { version: 1, process: 1 },
        asShot: { temperature: 4550, tint: -12 },
        presets,
        adapter: "Carte de test",
        decodeMs: 210,
      };
    if (cmd === "develop_render" || cmd === "develop_zoom_render") return frame(true);
    if (cmd === "develop_zoom_open") return { width: 6000, height: 4000, decodeMs: 900 };
    if (cmd === "develop_geometry_constrain") {
      // Le cœur rétrécit le cadre de 10 % quand l'image est redressée.
      const g = a.geometry as {
        crop: { left: number; top: number; right: number; bottom: number };
      };
      return { ...g, crop: { left: 0.05, top: 0.05, right: 0.95, bottom: 0.95 } };
    }
    if (cmd === "develop_auto_straighten") return -2.4;
    if (cmd === "develop_spot_source") return [0.123456789, 0.4];
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
  useDevelop.setState({
    clipping: { highlights: false, shadows: false },
    tool: "adjust",
    showDust: false,
  });
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
      clipping: { highlights: false, shadows: false, dust: false },
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
        version: "",
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
        version: "",
        settings: { version: 1, process: 1, light: { shadows: 40 } },
        label: "light.shadows",
      },
      {
        mediaId: 7,
        version: "",
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

  it("annuler, rétablir et revenir à une étape", async () => {
    const { canRedo, canUndo } = await import("./develop");
    await useDevelop.getState().open(7);
    expect(canUndo(useDevelop.getState())).toBe(false);
    useDevelop.getState().setLight("exposure", 0.5);
    // Réglage pas encore enregistré : annulable quand même.
    expect(canUndo(useDevelop.getState())).toBe(true);
    useDevelop.getState().setLight("contrast", 20);
    await vi.advanceTimersByTimeAsync(SAVE_DELAY_MS + 10);
    expect(useDevelop.getState().history?.current).toBe(2);

    await useDevelop.getState().undo();
    expect(useDevelop.getState().settings?.light).toEqual({ exposure: 0.5 });
    expect(canRedo(useDevelop.getState())).toBe(true);
    await useDevelop.getState().redo();
    expect(useDevelop.getState().settings?.light).toEqual({ exposure: 0.5, contrast: 20 });

    await useDevelop.getState().goTo(0);
    expect(useDevelop.getState().settings).toEqual({ version: 1, process: 1 });
    expect(canUndo(useDevelop.getState())).toBe(false);
    // Un nouveau réglage remplace les étapes annulées.
    useDevelop.getState().setColor("saturation", -30);
    await useDevelop.getState().flush();
    expect(useDevelop.getState().history?.steps.map((st) => st.label)).toEqual([
      "color.saturation",
    ]);
  });

  it("annuler enregistre d'abord le réglage en cours, puis l'annule", async () => {
    await useDevelop.getState().open(7);
    useDevelop.getState().setLight("whites", 15);
    await useDevelop.getState().undo();
    expect(saves()).toHaveLength(1);
    expect(useDevelop.getState().settings).toEqual({ version: 1, process: 1 });
    expect(useDevelop.getState().history?.current).toBe(0);
  });

  it("avant/après : l'original est rendu une fois, sans enregistrement", async () => {
    await useDevelop.getState().open(7);
    useDevelop.getState().setCompare("split");
    await vi.waitFor(() => expect(useDevelop.getState().beforeFrame).not.toBeNull());
    useDevelop.getState().setCompare("before");
    useDevelop.getState().setCompare("off");
    const neutralRenders = calls("develop_render").filter(
      ([, a]) =>
        JSON.stringify((a as { settings: object }).settings) === '{"version":1,"process":1}',
    );
    // Le rendu initial (photo neutre) et celui de l'original pour l'avant/après.
    expect(neutralRenders).toHaveLength(2);
    expect(saves()).toHaveLength(0);
  });

  it("instantané : garde les réglages affichés, nom libre suivant", async () => {
    const { freeName } = await import("./develop");
    await useDevelop.getState().open(7);
    useDevelop.getState().setLight("exposure", 0.8);
    await useDevelop.getState().createSnapshot("Instantané 1");
    // Le réglage en attente est enregistré avant l'instantané.
    expect(saves()).toHaveLength(1);
    expect(calls("develop_snapshot_create")[0]?.[1]).toEqual({
      mediaId: 7,
      version: "",
      name: "Instantané 1",
    });
    expect(useDevelop.getState().snapshots.map((x) => x.name)).toEqual(["Instantané 1"]);
    expect(freeName((n) => `Instantané ${n}`, ["Instantané 1", "Instantané 3"])).toBe(
      "Instantané 2",
    );
  });

  it("copie virtuelle : créée depuis la version affichée, puis affichée", async () => {
    await useDevelop.getState().open(7);
    await useDevelop.getState().createVersion(" Copie 1 ");
    expect(calls("develop_version_create")[0]?.[1]).toEqual({
      mediaId: 7,
      from: "",
      name: " Copie 1 ",
    });
    expect(useDevelop.getState().version).toBe("Copie 1");
    // Les réglages suivants vont à la copie.
    useDevelop.getState().setLight("contrast", 10);
    await useDevelop.getState().flush();
    expect(saves()[saves().length - 1]).toMatchObject({
      version: "Copie 1",
      label: "light.contrast",
    });
  });

  it("courbes, TSL et effets : JSON creux, une étape par réglage", async () => {
    const { IDENTITY_CURVE } = await import("./develop");
    await useDevelop.getState().open(7);
    const st = useDevelop.getState();
    st.setCurve("rgb", [
      [0, 10.4],
      [128, 140.6],
      [255, 255],
    ]);
    expect(useDevelop.getState().settings?.curve).toEqual({
      rgb: [
        [0, 10],
        [128, 141],
        [255, 255],
      ],
    });
    st.setCurve("rgb", IDENTITY_CURVE);
    expect(useDevelop.getState().settings?.curve).toBeUndefined();
    st.setHsl("saturation", "blue", -40);
    st.setHsl("saturation", "blue", 0);
    expect(useDevelop.getState().settings?.hsl).toBeUndefined();
    st.setEffect("vignette", "amount", -30);
    st.setEffect("vignette", "midpoint", 70);
    st.setEffect("vignette", "midpoint", 50);
    expect(useDevelop.getState().settings?.effects).toEqual({ vignette: { amount: -30 } });
    st.setDetail("sharpening", "radius", 1.43);
    st.setDetail("sharpening", "amount", 40);
    st.setDetail("sharpening", "radius", 1);
    expect(useDevelop.getState().settings?.detail).toEqual({ sharpening: { amount: 40 } });
    st.setEffect("grain", "size", 25);
    expect(useDevelop.getState().settings?.effects?.grain).toBeUndefined();
    await useDevelop.getState().flush();
    expect(saves().map((a) => (a as { label: string }).label)).toEqual([
      "curve.rgb",
      "hsl.saturation.blue",
      "effects.vignette.amount",
      "effects.vignette.midpoint",
      "detail.sharpening.radius",
      "detail.sharpening.amount",
      "detail.sharpening.radius",
      "effects.grain.size",
    ]);
  });

  it("zoom 100 % : centré sur le point visé, borné à l'image, suit les réglages", async () => {
    await useDevelop.getState().open(7);
    useDevelop.getState().setViewport(1000, 800);
    await useDevelop.getState().zoomAt(0.5, 0.25);
    expect(calls("develop_zoom_open")).toHaveLength(1);
    expect(useDevelop.getState().zoom).toEqual({ left: 2500, top: 600 });
    expect(calls("develop_zoom_render")[0]?.[1]).toMatchObject({
      left: 2500,
      top: 600,
      width: 1000,
      height: 800,
    });
    // Coin : la région reste dans l'image.
    await useDevelop.getState().zoomAt(1, 1);
    expect(useDevelop.getState().zoom).toEqual({ left: 5000, top: 3200 });
    // Le décodage pleine définition n'a lieu qu'une fois.
    expect(calls("develop_zoom_open")).toHaveLength(1);
    // Glisser vers la droite montre la partie gauche.
    useDevelop.getState().panZoom(300, 0);
    expect(useDevelop.getState().zoom).toEqual({ left: 4700, top: 3200 });
    useDevelop.getState().panZoom(-10000, -10000);
    expect(useDevelop.getState().zoom).toEqual({ left: 5000, top: 3200 });
    // Un réglage rend l'aperçu (histogramme) et la région.
    const before = calls("develop_zoom_render").length;
    useDevelop.getState().setLight("exposure", 0.5);
    await vi.waitFor(() => expect(calls("develop_zoom_render").length).toBeGreaterThan(before));
    useDevelop.getState().exitZoom();
    expect(useDevelop.getState().zoom).toBeNull();
  });

  it("le zoom se prépare en arrière-plan après un moment sur la photo", async () => {
    const { ZOOM_PREPARE_DELAY_MS } = await import("./develop");
    await useDevelop.getState().open(7);
    expect(calls("develop_zoom_open")).toHaveLength(0);
    await vi.advanceTimersByTimeAsync(ZOOM_PREPARE_DELAY_MS + 10);
    expect(calls("develop_zoom_open")).toHaveLength(1);
    expect(useDevelop.getState().fullSize).toEqual({ width: 6000, height: 4000 });
    // Le zoom ne décode pas une seconde fois.
    useDevelop.getState().setViewport(1000, 800);
    await useDevelop.getState().zoomAt(0.5, 0.5);
    expect(calls("develop_zoom_open")).toHaveLength(1);
  });

  it("passer vite d'une photo à l'autre ne prépare rien", async () => {
    const { ZOOM_PREPARE_DELAY_MS } = await import("./develop");
    await useDevelop.getState().open(7);
    await vi.advanceTimersByTimeAsync(ZOOM_PREPARE_DELAY_MS / 2);
    await useDevelop.getState().open(8);
    await vi.advanceTimersByTimeAsync(ZOOM_PREPARE_DELAY_MS / 2 + 10);
    expect(calls("develop_zoom_open")).toHaveLength(0);
  });

  it("changer de photo quitte le zoom", async () => {
    await useDevelop.getState().open(7);
    useDevelop.getState().setViewport(1000, 800);
    await useDevelop.getState().zoomAt(0.5, 0.5);
    await useDevelop.getState().open(8);
    expect(useDevelop.getState().zoom).toBeNull();
    expect(useDevelop.getState().fullSize).toBeNull();
  });

  it("outil de recadrage : l'image entière rendue, le cadre enregistré à part", async () => {
    await useDevelop.getState().open(7);
    await useDevelop.getState().setCrop({ left: 0.1, top: 0.2, right: 0.7, bottom: 0.9 });
    await useDevelop.getState().flush();
    const lastRender = () => {
      const all = calls("develop_render");
      return all[all.length - 1]?.[1] as { settings: { geometry?: object } } | undefined;
    };
    await useDevelop.getState().startCrop();
    await vi.waitFor(() => expect(useDevelop.getState().cropping).toBe(true));
    // Dans l'outil : rendu sans recadrage.
    await vi.waitFor(() => expect(lastRender()?.settings.geometry).toBeUndefined());
    useDevelop.getState().finishCrop();
    await vi.waitFor(() =>
      expect(lastRender()?.settings.geometry).toEqual({
        crop: { left: 0.1, top: 0.2, right: 0.7, bottom: 0.9 },
      }),
    );
  });

  it("barre d'outils : recadrer ouvre l'outil, un autre outil l'applique", async () => {
    await useDevelop.getState().open(7);
    await useDevelop.getState().setTool("crop");
    expect(useDevelop.getState()).toMatchObject({ tool: "crop", cropping: true });
    await useDevelop.getState().setTool("spots");
    expect(useDevelop.getState()).toMatchObject({ tool: "spots", cropping: false });
    // Changer de photo garde l'outil de correction, pas celui de recadrage.
    await useDevelop.getState().open(8);
    expect(useDevelop.getState().tool).toBe("spots");
  });

  it("correcteur : source choisie par le cœur, une étape par correction", async () => {
    await useDevelop.getState().open(7);
    await useDevelop.getState().setTool("spots");
    await useDevelop.getState().addSpot(0.5, 0.25);
    expect(calls("develop_spot_source")[0]?.[1]).toEqual({
      x: 0.5,
      y: 0.25,
      radius: 0.01,
      others: [],
    });
    expect(useDevelop.getState().settings?.spots).toEqual([
      { x: 0.5, y: 0.25, radius: 0.01, sourceX: 0.12346, sourceY: 0.4 },
    ]);
    expect(useDevelop.getState().selectedSpot).toBe(0);
    // La correction choisie suit le pinceau.
    useDevelop.getState().setSpotBrush({ mode: "clone" });
    await useDevelop.getState().moveSpot(0, "source", 0.7, 0.3);
    await useDevelop.getState().addSpot(0.1, 0.1);
    await useDevelop.getState().flush();
    expect(saves().map((a) => (a as { label: string }).label)).toEqual([
      "spots.add",
      "spots.mode",
      "spots.source",
      "spots.add",
    ]);
    expect(useDevelop.getState().settings?.spots?.[0]).toMatchObject({
      mode: "clone",
      sourceX: 0.7,
    });
    await useDevelop.getState().removeSpot(0);
    expect(useDevelop.getState().settings?.spots).toHaveLength(1);
    await useDevelop.getState().clearSpots();
    expect(useDevelop.getState().settings?.spots).toBeUndefined();
  });

  it("objectif : reconnu à l'ouverture, réglages creux", async () => {
    await useDevelop.getState().open(7);
    expect(useDevelop.getState().lensName).toBe("Sony FE 16-35mm f/2.8 GM");
    useDevelop.getState().setLens("distortion", 60.4);
    expect(useDevelop.getState().settings?.lens).toEqual({ distortion: 60 });
    useDevelop.getState().setLens("enabled", false);
    useDevelop.getState().setLens("distortion", 100);
    expect(useDevelop.getState().settings?.lens).toEqual({ enabled: false });
    useDevelop.getState().setLens("enabled", true);
    expect(useDevelop.getState().settings?.lens).toBeUndefined();
  });

  it("visualisation des poussières : demandée au rendu dans l'outil seulement", async () => {
    await useDevelop.getState().open(7);
    useDevelop.getState().setShowDust(true);
    const lastClipping = () => {
      const all = calls("develop_render");
      return (all[all.length - 1]?.[1] as { clipping: { dust: boolean } }).clipping.dust;
    };
    await vi.waitFor(() => expect(lastClipping()).toBe(false));
    await useDevelop.getState().setTool("spots");
    await vi.waitFor(() => expect(lastClipping()).toBe(true));
  });

  it("quart de tour et miroir font suivre le cadre ; le redressement le contraint", async () => {
    await useDevelop.getState().open(7);
    await useDevelop.getState().setCrop({ left: 0.1, top: 0.2, right: 0.5, bottom: 0.6 });
    useDevelop.getState().rotateQuarter(1);
    expect(useDevelop.getState().settings?.geometry).toEqual({
      quarter: 1,
      crop: { left: 0.4, top: 0.1, right: 0.8, bottom: 0.5 },
    });
    useDevelop.getState().rotateQuarter(-1);
    expect(useDevelop.getState().settings?.geometry?.crop).toEqual({
      left: 0.1,
      top: 0.2,
      right: 0.5,
      bottom: 0.6,
    });
    expect(useDevelop.getState().settings?.geometry?.quarter).toBeUndefined();
    useDevelop.getState().flip("h");
    expect(useDevelop.getState().settings?.geometry).toMatchObject({
      flipH: true,
      crop: { left: 0.5, right: 0.9 },
    });
    await useDevelop.getState().autoStraighten();
    expect(useDevelop.getState().settings?.geometry).toMatchObject({
      angle: -2.4,
      crop: { left: 0.05, right: 0.95 },
    });
    await useDevelop.getState().flush();
    expect(saves()[saves().length - 1]).toMatchObject({ label: "geometry.auto" });
  });

  it("annuler le recadrage revient à l'état d'avant l'outil", async () => {
    await useDevelop.getState().open(7);
    useDevelop.getState().setLight("exposure", 0.5);
    await useDevelop.getState().startCrop();
    await useDevelop.getState().setCrop({ left: 0.2, top: 0.2, right: 0.8, bottom: 0.8 });
    useDevelop.getState().rotateQuarter(1);
    await useDevelop.getState().cancelCrop();
    expect(useDevelop.getState().cropping).toBe(false);
    expect(useDevelop.getState().settings).toEqual({
      version: 1,
      process: 1,
      light: { exposure: 0.5 },
    });
  });

  it("une nouvelle géométrie quitte le zoom", async () => {
    await useDevelop.getState().open(7);
    useDevelop.getState().setViewport(1000, 800);
    await useDevelop.getState().zoomAt(0.5, 0.5);
    useDevelop.getState().flip("v");
    expect(useDevelop.getState().zoom).toBeNull();
    expect(useDevelop.getState().fullSize).toBeNull();
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
