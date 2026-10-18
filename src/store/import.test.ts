import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ImportOptions, SourceView } from "../api/core";

const importStart = vi.fn<(o: ImportOptions) => Promise<number>>();

vi.mock("../api/core", async (original) => ({
  ...(await original<typeof import("../api/core")>()),
  api: {
    sourceScan: vi.fn(async (): Promise<SourceView> => source),
    importStart: (o: ImportOptions) => importStart(o),
  },
}));

const { useImport } = await import("./import");

const entry = (path: string, alreadyImported = false) => ({
  path,
  fileName: path.split("/").pop() ?? path,
  size: 1,
  format: "raw" as const,
  capturedAt: "2026-10-18T21:04:59",
  camera: "X-T3",
  alreadyImported,
});

const source: SourceView = {
  root: "/carte",
  ignored: 0,
  days: [
    { day: "2026-10-17", files: [entry("/carte/a.RAF", true), entry("/carte/b.RAF")] },
    { day: "2026-10-18", files: [entry("/carte/c.RAF"), entry("/carte/d.RAF")] },
  ],
};

describe("store d'import", () => {
  beforeEach(() => {
    importStart.mockReset().mockResolvedValue(1);
    useImport.setState({
      libraries: [
        {
          id: 7,
          name: "Photos",
          rootPath: "/bib",
          backupPath: null,
          folderTemplate: "{yyyy}",
          renameTemplate: null,
          isOnline: true,
        },
      ],
      libraryId: 7,
      source: null,
      selected: new Set(),
      running: false,
    });
  });

  it("sélectionne par défaut tout ce qui n'est pas déjà importé", async () => {
    await useImport.getState().scanSource("/carte");
    expect([...useImport.getState().selected].sort()).toEqual([
      "/carte/b.RAF",
      "/carte/c.RAF",
      "/carte/d.RAF",
    ]);
  });

  it("une photo déjà importée reste hors sélection, même par jour", async () => {
    await useImport.getState().scanSource("/carte");
    useImport.getState().setDaySelected("2026-10-17", false);
    useImport.getState().setDaySelected("2026-10-17", true);
    expect(useImport.getState().selected.has("/carte/a.RAF")).toBe(false);
    expect(useImport.getState().selected.has("/carte/b.RAF")).toBe(true);
  });

  it("envoie les options choisies au cœur", async () => {
    await useImport.getState().scanSource("/carte");
    useImport.getState().toggle("/carte/d.RAF");
    useImport.getState().updateForm({
      albumName: "  Sortie  ",
      keywords: "mer, , Bretagne ",
      rights: "",
      backup: true,
    });
    await useImport.getState().start("carte");

    const options = importStart.mock.calls[0]?.[0];
    expect(options).toMatchObject({
      libraryId: 7,
      sourcePath: "/carte",
      albumName: "Sortie",
      keywords: ["mer", "Bretagne"],
      rights: null,
      // Pas de dossier de sauvegarde dans cette bibliothèque.
      backup: false,
    });
    expect(options?.files.sort()).toEqual(["/carte/b.RAF", "/carte/c.RAF"]);
    expect(useImport.getState().running).toBe(true);
  });
});
