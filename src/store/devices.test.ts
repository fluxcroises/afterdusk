import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn<(cmd: string, args?: Record<string, unknown>) => Promise<unknown>>();
vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: (cmd: string, args?: Record<string, unknown>) => invoke(cmd, args),
}));

const { default: i18n } = await import("../i18n");
await i18n.changeLanguage("fr");
const { useDevices, deviceLabel } = await import("./devices");
const { useImport } = await import("./import");
const { useNavigation } = await import("./navigation");

const card = { root: "E:\\", volume: "E:", camera: "FUJI" };

function behave(behavior: string) {
  invoke.mockReset().mockImplementation((cmd) => {
    if (cmd === "insert_behavior_get") return Promise.resolve(behavior);
    if (cmd === "source_scan") return Promise.resolve({ root: "E:\\", days: [], ignored: 0 });
    return Promise.resolve(null);
  });
}

describe("carte insérée", () => {
  beforeEach(() => {
    useDevices.setState({ devices: [], pending: null, behavior: "propose" });
    useImport.setState({ running: false, source: null, libraryId: 1 });
    useNavigation.setState({ screen: "library" });
  });

  it("libellé : marque devinée et lecteur", () => {
    expect(deviceLabel(card)).toBe("Carte FUJI (E:)");
    expect(deviceLabel({ ...card, camera: null })).toBe("Carte (E:)");
  });

  it("proposer : un bandeau, retiré si la carte est retirée", async () => {
    behave("propose");
    await useDevices.getState().attached(card, "main");
    expect(useDevices.getState().devices).toEqual([card]);
    expect(useDevices.getState().pending).toEqual(card);
    expect(useNavigation.getState().screen).toBe("library");
    useDevices.getState().detached(card);
    expect(useDevices.getState().pending).toBeNull();
    expect(useDevices.getState().devices).toEqual([]);
  });

  it("ouvrir : Importer s'affiche, la carte analysée", async () => {
    behave("open");
    await useDevices.getState().attached(card, "main");
    expect(useNavigation.getState().screen).toBe("import");
    expect(invoke).toHaveBeenCalledWith("source_scan", { path: "E:\\", libraryId: 1 });
    expect(useDevices.getState().pending).toBeNull();
  });

  it("ouvrir pendant un import : proposé seulement, l'import continue", async () => {
    behave("open");
    useImport.setState({ running: true });
    await useDevices.getState().attached(card, "main");
    expect(useNavigation.getState().screen).toBe("library");
    expect(useDevices.getState().pending).toEqual(card);
  });

  it("ne rien faire : la carte est listée, sans bandeau", async () => {
    behave("nothing");
    await useDevices.getState().attached(card, "main");
    expect(useDevices.getState().devices).toEqual([card]);
    expect(useDevices.getState().pending).toBeNull();
  });
});
