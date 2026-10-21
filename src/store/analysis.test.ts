import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn<(cmd: string, args?: Record<string, unknown>) => Promise<unknown>>();
vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: (cmd: string, args?: Record<string, unknown>) => invoke(cmd, args),
}));

const { useLibrary } = await import("./library");
const { useAnalysis, missesOf } = await import("./analysis");
const { useBursts } = await import("./bursts");
const { useImport } = await import("./import");

describe("analyse des ratés", () => {
  beforeEach(() => {
    invoke.mockReset().mockImplementation((cmd) => {
      if (cmd === "analysis_run") return Promise.resolve({ analyzed: 8, misses: 2, skipped: 0 });
      if (cmd === "bursts_detect") return Promise.resolve({ bursts: 0, photos: 0 });
      if (cmd === "media_list") return Promise.resolve([]);
      return Promise.resolve([]);
    });
    useAnalysis.setState({ running: false, progress: null, lastReport: null, auto: true });
    useBursts.setState({ running: false, auto: true });
    useLibrary.setState({ libraryId: 1, albumId: null, filter: {}, media: [], error: null });
  });

  it("analyse l'album affiché et recharge la grille", async () => {
    await useAnalysis.getState().run(4);
    expect(invoke).toHaveBeenCalledWith("analysis_run", { libraryId: 1, albumId: 4 });
    expect(useAnalysis.getState().lastReport).toEqual({ analyzed: 8, misses: 2, skipped: 0 });
    expect(useAnalysis.getState().running).toBe(false);
    expect(invoke.mock.calls.some(([cmd]) => cmd === "media_list")).toBe(true);
  });

  it("raisons lues depuis la grille", () => {
    expect(missesOf("blur over")).toEqual(["blur", "over"]);
    expect(missesOf("")).toEqual([]);
    expect(missesOf(undefined)).toEqual([]);
  });

  it("après un import : ratés d'abord, rafales ensuite (la netteté choisit le dessus)", async () => {
    useImport.getState().onFinished({
      importId: 3,
      done: 8,
      skipped: 0,
      failed: 0,
      error: null,
    } as never);
    await vi.waitFor(() =>
      expect(invoke.mock.calls.some(([cmd]) => cmd === "bursts_detect")).toBe(true),
    );
    const order = invoke.mock.calls
      .map(([cmd]) => cmd)
      .filter((c) => c === "analysis_run" || c === "bursts_detect");
    expect(order).toEqual(["analysis_run", "bursts_detect"]);
  });

  it("les deux se coupent dans les Réglages", async () => {
    useAnalysis.getState().setAuto(false);
    useBursts.getState().setAuto(false);
    useImport.getState().onFinished({
      importId: 3,
      done: 8,
      skipped: 0,
      failed: 0,
      error: null,
    } as never);
    await new Promise((r) => setTimeout(r, 0));
    expect(
      invoke.mock.calls.some(([cmd]) => cmd === "analysis_run" || cmd === "bursts_detect"),
    ).toBe(false);
    useAnalysis.getState().setAuto(true);
    useBursts.getState().setAuto(true);
  });
});
