import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn<(cmd: string, args?: Record<string, unknown>) => Promise<unknown>>();
vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: (cmd: string, args?: Record<string, unknown>) => invoke(cmd, args),
}));

const confirmWarning = vi.fn(() => Promise.resolve(true));
vi.mock("../ui/confirm", () => ({ confirmWarning: () => confirmWarning() }));

const { default: i18n } = await import("../i18n");
await i18n.changeLanguage("fr");
const { useGear } = await import("../store/gear");
const { useReminders } = await import("../store/maintenance");
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

const journal = [
  {
    id: 7,
    gearId: 1,
    date: "2026-01-10",
    kind: "sensor",
    costCents: 4500,
    currency: "EUR",
    notes: "Chez le réparateur",
    reminderMonths: 6,
  },
];
const reminders = [
  {
    gearId: 1,
    gearName: "Nikon Z 6",
    kind: "sensor",
    lastDate: "2026-01-10",
    dueDate: "2026-07-10",
    due: true,
  },
];

const stats = {
  photos: 12,
  focal: [
    { value: 24, count: 7 },
    { value: 70, count: 5 },
  ],
  aperture: [{ value: 4, count: 12 }],
  iso: [
    { value: 100, count: 8 },
    { value: 3200, count: 4 },
  ],
  months: [
    { month: "2026-07", count: 10 },
    { month: "2026-09", count: 2 },
  ],
  partners: [{ id: 3, name: "NIKKOR Z 24-70mm f/4 S", count: 12 }],
  albums: [{ albumId: 9, name: "Islande", count: 12, favoriteLens: "NIKKOR Z 24-70mm f/4 S" }],
  shutter: { read: 739, readAt: "2026-07-14T10:00:00", since: 11, estimate: 750 },
};

function closest(text: string, selector: string): HTMLElement {
  const el = screen.getByText(text).closest(selector);
  if (!(el instanceof HTMLElement)) throw new Error(`${text} hors de ${selector}`);
  return el;
}

describe("écran Matériel", () => {
  beforeEach(() => {
    invoke.mockReset().mockImplementation((cmd, args) => {
      if (cmd === "gear_list") return Promise.resolve(items);
      if (cmd === "gear_get") return Promise.resolve(record(args?.id as number));
      if (cmd === "gear_create") return Promise.resolve(4);
      if (cmd === "gear_stats") return Promise.resolve(stats);
      if (cmd === "maintenance_list") return Promise.resolve(journal);
      if (cmd === "maintenance_reminders")
        return Promise.resolve(args?.gearId === null || args?.gearId === 1 ? reminders : []);
      return Promise.resolve(null);
    });
    useGear.setState({ items: [], selectedId: null, record: null, showHidden: false });
    useReminders.setState({ reminders: [] });
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

  it("statistiques et compteur de déclenchements", async () => {
    await act(async () => render(<GearView />));
    expect(screen.getByRole("heading", { name: "Statistiques" })).toBeInTheDocument();
    // Sans survol, la barre la plus haute ; au survol, celle sous la souris.
    const focal = closest("Focales", "figure");
    expect(within(focal).getByText("24 mm · 7 photos")).toBeInTheDocument();
    fireEvent.mouseEnter(focal.querySelectorAll("svg g")[1]);
    expect(within(focal).getByText("70 mm · 5 photos")).toBeInTheDocument();
    // Le mois sans photo compte, à zéro.
    const months = closest("Photos dans le temps", "figure");
    expect(months.querySelectorAll("svg path")).toHaveLength(3);
    expect(screen.getByText("Déclenchements").nextSibling).toHaveTextContent("750");
    expect(
      screen.getByText(/^739 lus dans la photo du .*, plus 11 photos prises depuis\.$/),
    ).toBeInTheDocument();
    // Un objectif associé ouvre sa fiche.
    await act(async () =>
      fireEvent.click(
        within(closest("Objectifs utilisés avec", "div")).getByRole("button", {
          name: /NIKKOR Z 24-70mm f\/4 S.*12/,
        }),
      ),
    );
    expect(invoke).toHaveBeenCalledWith("gear_get", { id: 3 });
  });

  it("journal d'entretien, rappel échu, ajout et suppression", async () => {
    await act(async () => render(<GearView />));
    await act(async () => useReminders.getState().load());
    const card = closest("Entretien", "section");
    expect(
      within(card).getByText("Nettoyage du capteur : à faire depuis le 10 juil. 2026"),
    ).toBeInTheDocument();
    expect(within(card).getByText(/45,00/)).toBeInTheDocument();
    expect(within(card).getByText("tous les 6 mois · Chez le réparateur")).toBeInTheDocument();
    // Repère dans la liste.
    expect(screen.getByRole("button", { name: /Nikon Z 6\s*entretien/ })).toBeInTheDocument();

    fireEvent.click(within(card).getByRole("button", { name: "Ajouter une intervention…" }));
    fireEvent.change(within(card).getByLabelText("Date"), { target: { value: "2026-10-20" } });
    fireEvent.change(within(card).getByLabelText("Intervention"), {
      target: { value: "firmware" },
    });
    fireEvent.change(within(card).getByLabelText("Rappel"), { target: { value: "12" } });
    await act(async () => fireEvent.click(within(card).getByRole("button", { name: "Ajouter" })));
    expect(invoke).toHaveBeenCalledWith("maintenance_add", {
      gearId: 1,
      input: {
        date: "2026-10-20",
        kind: "firmware",
        costCents: null,
        currency: null,
        notes: null,
        reminderMonths: 12,
      },
    });

    await act(async () =>
      fireEvent.click(within(card).getByRole("button", { name: "Supprimer l'intervention" })),
    );
    expect(confirmWarning).toHaveBeenCalled();
    expect(invoke).toHaveBeenCalledWith("maintenance_delete", { id: 7 });
  });
});
