import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn<(cmd: string, args?: Record<string, unknown>) => Promise<unknown>>();
vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: (cmd: string, args?: Record<string, unknown>) => invoke(cmd, args),
}));

const { default: i18n } = await import("../i18n");
await i18n.changeLanguage("fr");
const { useGear } = await import("../store/gear");
const { GearView } = await import("./GearView");

const item = (id: number, kind: string, make: string, model: string, extra = {}) => ({
  id,
  kind,
  make,
  model,
  displayName: make ? `${make} ${model}` : model,
  style: kind === "body" ? "mirrorless" : kind === "lens" ? "zoom" : null,
  serial: null,
  autoDetected: kind === "body" || kind === "lens",
  hidden: false,
  confirmed: false,
  photoMediaId: null,
  hasPhotoFile: false,
  photoCount: kind === "body" || kind === "lens" ? 12 : 0,
  firstUsed: "2026-07-14T10:00:00",
  lastUsed: "2026-08-01T18:30:00",
  ...extra,
});

const items = [
  item(1, "body", "Nikon", "Z 6"),
  item(2, "body", "Nikon", "Z 6 II"),
  item(3, "lens", "", "NIKKOR Z 24-70mm f/4 S"),
  item(4, "tripod", "Manfrotto", "Befree"),
];

const record = (id: number) => ({
  ...items.find((i) => i.id === id),
  name: null,
  purchaseDate: null,
  purchasePlace: null,
  priceCents: null,
  currency: null,
  condition: null,
  warrantyUntil: null,
  notes: null,
  aliases: id === 1 ? ["NIKON CORPORATION NIKON Z 6 (6012345)"] : [],
});

describe("écran Matériel", () => {
  beforeEach(() => {
    invoke.mockReset().mockImplementation((cmd, args) => {
      if (cmd === "gear_list") return Promise.resolve(items);
      if (cmd === "gear_get") return Promise.resolve(record(args?.id as number));
      if (cmd === "gear_create") return Promise.resolve(4);
      return Promise.resolve(null);
    });
    useGear.setState({ items: [], selectedId: null, record: null, showHidden: false });
  });

  it("inventaire par type, fiche et silhouette du premier boîtier", async () => {
    await act(async () => render(<GearView />));
    expect(screen.getByText("Boîtiers")).toBeInTheDocument();
    expect(screen.getByText("Accessoires")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Nikon Z 6" })).toBeInTheDocument();
    // Silhouette (pas de photo) et marque en toutes lettres.
    expect(document.querySelector("svg.gear-silhouette")).not.toBeNull();
    expect(screen.getByText("NIKON CORPORATION NIKON Z 6 (6012345)")).toBeInTheDocument();
    expect(screen.getByText("Photos").nextSibling).toHaveTextContent("12");
  });

  it("enregistre un champ, masque et fusionne", async () => {
    await act(async () => render(<GearView />));
    const name = screen.getByLabelText("Nom");
    fireEvent.change(name, { target: { value: "Boîtier principal" } });
    await act(async () => fireEvent.blur(name));
    const update = invoke.mock.calls.find(([cmd]) => cmd === "gear_update")?.[1];
    expect(update).toMatchObject({ id: 1, update: { name: "Boîtier principal" } });

    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Masquer" })));
    expect(invoke).toHaveBeenCalledWith("gear_set_hidden", { id: 1, hidden: true });

    // Fusion : seulement avec un appareil du même type.
    const merge = screen.getByLabelText("Fusionner avec…");
    expect(within(merge).queryByText("Manfrotto Befree")).not.toBeInTheDocument();
    fireEvent.change(merge, { target: { value: "2" } });
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Fusionner" })));
    expect(invoke).toHaveBeenCalledWith("gear_merge", { source: 1, target: 2 });
  });

  it("ajoute un accessoire, saisi à la main", async () => {
    await act(async () => render(<GearView />));
    await act(async () =>
      fireEvent.change(screen.getByLabelText("Ajouter…"), { target: { value: "tripod" } }),
    );
    expect(invoke).toHaveBeenCalledWith("gear_create", { kind: "tripod", make: "", model: "" });
    expect(screen.getByRole("heading", { name: "Manfrotto Befree" })).toBeInTheDocument();
    expect(screen.getByText("Saisi à la main.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Supprimer" })).toBeInTheDocument();
  });

  it("un accessoire sans marque ni modèle prend le nom de son type", async () => {
    invoke.mockImplementation((cmd) => {
      const blank = { ...item(5, "tripod", "", ""), displayName: "" };
      if (cmd === "gear_list") return Promise.resolve([blank]);
      if (cmd === "gear_get") return Promise.resolve({ ...record(4), ...blank, aliases: [] });
      return Promise.resolve(null);
    });
    await act(async () => render(<GearView />));
    expect(screen.getByRole("heading", { name: "Trépied" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Trépied" })).toBeInTheDocument();
  });
});
