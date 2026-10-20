import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: vi.fn(),
}));

const { invoke } = await import("@tauri-apps/api/core");
const { useDeliver } = await import("./deliver");
const mocked = vi.mocked(invoke);

const preset = (id: string) => ({
  id,
  name: null,
  output: { kind: "jpeg", quality: 85 },
  resize: { mode: "longSide", value: 2048 },
  color: "srgb",
  sharpen: "screen",
  metadata: "noGps",
  watermark: false,
  naming: "{name}",
});

let saved: unknown = null;

beforeEach(() => {
  vi.useFakeTimers();
  mocked.mockReset();
  saved = null;
  mocked.mockImplementation(async (cmd: string, args?: unknown) => {
    const a = (args ?? {}) as Record<string, unknown>;
    if (cmd === "export_settings")
      return {
        version: 1,
        presets: [preset("web"), preset("print")],
        watermark: null,
        destination: "D:/Livraison",
        openFolder: true,
      };
    if (cmd === "export_settings_save") {
      saved = a.settings;
      const s = a.settings as { presets: { id: string }[] };
      // Le cœur rétablit un preset fourni retiré.
      if (!s.presets.some((p) => p.id === "print")) s.presets.push(preset("print"));
      return s;
    }
    return null;
  });
  useDeliver.setState({ chosen: ["web"], running: false, progress: null, finished: null });
});

describe("Livrer", () => {
  it("exporte la sélection avec les presets cochés, en sous-dossiers nommés", async () => {
    await useDeliver.getState().load();
    useDeliver.getState().toggleChosen("print");
    await useDeliver.getState().start([3, 4], { web: "Web", print: "Impression" });
    expect(mocked.mock.calls.find(([c]) => c === "export_start")?.[1]).toEqual({
      request: {
        mediaIds: [3, 4],
        targets: [
          { preset: "web", folder: "Web" },
          { preset: "print", folder: "Impression" },
        ],
        destination: "D:/Livraison",
      },
    });
    expect(useDeliver.getState().progress?.total).toBe(4);
    expect(useDeliver.getState().running).toBe(true);
    useDeliver.getState().onFinished({
      progress: { done: 4, total: 4, current: null, failures: [] },
      cancelled: false,
      destination: "D:/Livraison",
      error: null,
    });
    expect(useDeliver.getState().running).toBe(false);
  });

  it("options enregistrées peu après ; un preset fourni se rétablit", async () => {
    await useDeliver.getState().load();
    useDeliver.getState().updatePreset("web", { naming: "{date}-{seq}" });
    expect(saved).toBeNull();
    await vi.advanceTimersByTimeAsync(600);
    expect((saved as { presets: { naming: string }[] }).presets[0]?.naming).toBe("{date}-{seq}");
    useDeliver.getState().duplicatePreset("web", "Client");
    const presets = useDeliver.getState().settings?.presets ?? [];
    expect(presets[presets.length - 1]).toMatchObject({ id: "user-1", name: "Client" });
    await useDeliver.getState().removePreset("print");
    expect(useDeliver.getState().settings?.presets.map((p) => p.id)).toContain("print");
  });
});
