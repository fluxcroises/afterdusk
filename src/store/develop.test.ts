import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: vi.fn(),
}));

const { invoke } = await import("@tauri-apps/api/core");
const { parseFrame, SAVE_DELAY_MS, useDevelop } = await import("./develop");

/** Rendu factice de 2 × 1 pixels, comme le renvoie `develop_render`. */
function frame(gpu: boolean) {
  const buffer = new ArrayBuffer(16 + 8);
  const view = new DataView(buffer);
  view.setUint32(0, 2, true);
  view.setUint32(4, 1, true);
  view.setUint32(8, 1500, true);
  view.setUint32(12, gpu ? 1 : 0, true);
  new Uint8Array(buffer, 16).set([10, 20, 30, 255, 40, 50, 60, 255]);
  return buffer;
}

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
        adapter: "Carte de test",
        decodeMs: 210,
      };
    if (cmd === "develop_render") return frame(true);
    return null;
  });
});

afterEach(async () => {
  await useDevelop.getState().close();
  vi.useRealTimers();
});

const calls = (cmd: string) => mocked.mock.calls.filter(([c]) => c === cmd);

describe("Retoucher", () => {
  it("lit l'en-tête et les pixels du rendu", () => {
    const f = parseFrame(frame(false));
    expect(f).toMatchObject({ width: 2, height: 1, renderMs: 1.5, gpu: false });
    expect(Array.from(f.pixels.slice(4, 7))).toEqual([40, 50, 60]);
  });

  it("ouvre la photo et affiche le rendu neutre", async () => {
    await useDevelop.getState().open(7);
    const s = useDevelop.getState();
    expect(s.settings).toEqual({ version: 1, process: 1 });
    expect(s.frame?.gpu).toBe(true);
    expect(calls("develop_render")[0]?.[1]).toEqual({ settings: { version: 1, process: 1 } });
  });

  it("rend à chaque geste et n'enregistre qu'une fois, après le dernier", async () => {
    await useDevelop.getState().open(7);
    useDevelop.getState().setExposure(0.3);
    useDevelop.getState().setExposure(0.7);
    useDevelop.getState().setExposure(1.234);
    await vi.waitFor(() => expect(useDevelop.getState().settings?.light?.exposure).toBe(1.23));
    expect(calls("develop_save")).toHaveLength(0);
    await vi.advanceTimersByTimeAsync(SAVE_DELAY_MS + 10);
    expect(calls("develop_save")).toEqual([
      [
        "develop_save",
        {
          mediaId: 7,
          settings: { version: 1, process: 1, light: { exposure: 1.23 } },
          label: "light.exposure",
        },
      ],
    ]);
    // Les gestes rapprochés ne lancent pas un rendu chacun.
    expect(calls("develop_render").length).toBeLessThanOrEqual(3);
  });

  it("revenir à zéro retire l'exposition du JSON creux", async () => {
    await useDevelop.getState().open(7);
    useDevelop.getState().setExposure(1);
    useDevelop.getState().setExposure(0);
    expect(useDevelop.getState().settings).toEqual({ version: 1, process: 1, light: {} });
  });

  it("changer de photo enregistre le réglage en attente", async () => {
    await useDevelop.getState().open(7);
    useDevelop.getState().setExposure(-0.5);
    await useDevelop.getState().open(8);
    expect(calls("develop_save")[0]?.[1]).toMatchObject({
      mediaId: 7,
      settings: { light: { exposure: -0.5 } },
    });
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
