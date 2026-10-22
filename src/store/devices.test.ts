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

function behave(behavior: string, auto: unknown = { kind: "unknown" }) {
  invoke.mockReset().mockImplementation((cmd) => {
    if (cmd === "insert_behavior_get") return Promise.resolve(behavior);
    if (cmd === "auto_import_card") return Promise.resolve(auto);
    if (cmd === "source_scan") return Promise.resolve({ root: "E:\\", days: [], ignored: 0 });
    return Promise.resolve(null);
  });
}

describe("carte insérée", () => {
  beforeEach(() => {
    useDevices.setState({ devices: [], pending: null, auto: null, behavior: "propose" });
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

  it("carte d'un boîtier connu : l'import part seul, sans bandeau d'import", async () => {
    behave("propose", { kind: "started", camera: "FUJIFILM X-T5", importId: 12, files: 23 });
    await useDevices.getState().attached(card, "main");
    expect(invoke).toHaveBeenCalledWith("auto_import_card", {
      root: "E:\\",
      sourceLabel: "Carte FUJI (E:)",
    });
    expect(useDevices.getState().auto).toEqual({
      kind: "started",
      camera: "FUJIFILM X-T5",
      importId: 12,
      files: 23,
    });
    expect(useDevices.getState().pending).toBeNull();
    expect(useImport.getState().running).toBe(true);
  });

  it("carte connue déjà importée : rien de nouveau, rien d'ouvert", async () => {
    behave("open", { kind: "nothingNew", camera: "FUJIFILM X-T5" });
    await useDevices.getState().attached(card, "main");
    expect(useDevices.getState().auto).toEqual({ kind: "nothingNew", camera: "FUJIFILM X-T5" });
    expect(useNavigation.getState().screen).toBe("library");
  });

  it("ne rien faire : même une carte connue n'est pas lue", async () => {
    behave("nothing", { kind: "started", camera: "X", importId: 1, files: 1 });
    await useDevices.getState().attached(card, "main");
    expect(invoke).not.toHaveBeenCalledWith("auto_import_card", expect.anything());
    expect(useDevices.getState().auto).toBeNull();
  });
});
