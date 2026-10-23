import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn<(cmd: string, args?: Record<string, unknown>) => Promise<unknown>>();
vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: (cmd: string, args?: Record<string, unknown>) => invoke(cmd, args),
}));
vi.mock("./platform", async (original) => ({
  ...(await original<typeof import("./platform")>()),
  isMac: false,
}));

const { default: i18n } = await import("./i18n");
await i18n.changeLanguage("fr");
const t = i18n.t.bind(i18n);
const { developMenuItems, photoMenuItems } = await import("./contextMenu");
const { useDevelop } = await import("./store/develop");
const { useLibrary } = await import("./store/library");
const { useNavigation } = await import("./store/navigation");

type Entry = {
  text?: string;
  items?: Entry[];
  item?: string;
  enabled?: boolean;
  action?: () => void;
};

/** Libellés de premier niveau, séparateurs notés « — ». */
const labels = (items: unknown[]) =>
  (items as Entry[]).map((i) => (i.item === "Separator" ? "—" : (i.text ?? "")));

const photo = (patch: Record<string, unknown> = {}) => ({
  id: 7,
  albumId: 1,
  fileName: "DSCF1667.JPG",
  capturedAt: null,
  rating: 0,
  flag: 0,
  color: null,
  width: 7728,
  height: 5152,
  orientation: 1,
  hasPreview: true,
  trashedAt: null,
  stackSize: 1,
  companionOf: null,
  ...patch,
});

describe("clic droit", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue(null);
    useLibrary.setState({ trash: false, selectedId: 7, selectedIds: [7] });
    useDevelop.setState({ clipboard: null });
    useNavigation.setState({ screen: "cull" });
  });

  it("sur une photo : visionneuse, Retoucher, marques, Explorateur, corbeille", () => {
    const items = photoMenuItems(t, photo());
    expect(labels(items)).toEqual([
      "Afficher dans la visionneuse\tEntrée",
      "Retoucher",
      "—",
      "Note",
      "Drapeau",
      "Étiquette de couleur",
      "—",
      "Définir comme couverture de l'album",
      "—",
      "Afficher dans l'Explorateur",
      "—",
      "Envoyer dans la corbeille\tSuppr",
    ]);
    // Les touches du tri, sans en faire des accélérateurs.
    const flags = (items as Entry[]).find((i) => i.text === "Drapeau")?.items ?? [];
    expect(labels(flags)).toEqual(["Retenue\tP", "Rejetée\tX", "Sans drapeau\tU"]);
    (items as Entry[]).find((i) => i.text === "Afficher dans l'Explorateur")?.action?.();
    expect(invoke).toHaveBeenCalledWith("media_reveal", { mediaId: 7 });
  });

  it("une pile propose ses formats ; une copie virtuelle n'a pas de fichier", () => {
    expect(labels(photoMenuItems(t, photo({ stackSize: 2 })))).toContain(
      "Envoyer le JPEG dans la corbeille",
    );
    const copy = labels(photoMenuItems(t, photo({ id: -3, copyOf: 7 })));
    expect(copy).not.toContain("Afficher dans l'Explorateur");
    expect(copy).not.toContain("Envoyer dans la corbeille\tSuppr");
  });

  it("des réglages copiés se collent depuis la grille", () => {
    expect(labels(photoMenuItems(t, photo()))).not.toContain("Coller les réglages\tCtrl+Maj+V");
    useDevelop.setState({ clipboard: { settings: {} as never, controls: [] } });
    expect(labels(photoMenuItems(t, photo()))).toContain("Coller les réglages\tCtrl+Maj+V");
  });

  it("dans la corbeille : restaurer ou supprimer définitivement", () => {
    useLibrary.setState({ trash: true });
    expect(labels(photoMenuItems(t, photo()))).toEqual(["Restaurer", "Supprimer définitivement…"]);
  });

  it("dans le Retoucher : avant/après, zoom, copier et coller", () => {
    useDevelop.setState({ settings: null, clipboard: null, zoom: null });
    const items = developMenuItems(t) as Entry[];
    expect(labels(items)).toEqual([
      "Avant / après\t\\",
      "Zoom 100 % (Z)",
      "—",
      "Copier les réglages…\tCtrl+Maj+C",
      "Coller les réglages\tCtrl+Maj+V",
    ]);
    // Rien à copier ni à coller sans photo ouverte.
    expect(items[3]?.enabled).toBe(false);
    expect(items[4]?.enabled).toBe(false);
  });
});
