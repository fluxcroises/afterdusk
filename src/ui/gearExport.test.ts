import { describe, expect, it } from "vitest";
import i18n from "../i18n";
import type { GearRecord } from "../store/gear";
import { csvDelimiter, type GearExportData, inventoryCsv, inventoryPdf } from "./gearExport";

await i18n.changeLanguage("fr");
const t = i18n.t.bind(i18n);

const record = (extra: Partial<GearRecord>): GearRecord => ({
  id: 1,
  kind: "body",
  make: "Nikon",
  model: "Z 6",
  displayName: "Nikon Z 6",
  style: "mirrorless",
  serial: "6035906",
  autoDetected: true,
  hidden: false,
  confirmed: true,
  photoMediaId: null,
  hasPhotoFile: false,
  photoCount: 1254,
  firstUsed: "2024-03-12T10:00:00",
  lastUsed: "2026-10-01T18:30:00",
  name: null,
  purchaseDate: "2024-03-12",
  purchasePlace: "Darty",
  priceCents: 189900,
  currency: "EUR",
  condition: "excellent",
  warrantyUntil: null,
  notes: null,
  aliases: [],
  ...extra,
});

const data: GearExportData[] = [
  {
    record: record({}),
    journal: [
      {
        id: 7,
        gearId: 1,
        date: "2026-01-10",
        kind: "sensor",
        costCents: 4500,
        currency: "EUR",
        notes: null,
        reminderMonths: 6,
      },
    ],
    shutter: 12480,
  },
  {
    record: record({
      id: 2,
      kind: "tripod",
      make: "",
      model: "",
      displayName: "",
      autoDetected: false,
      serial: null,
      purchaseDate: null,
      purchasePlace: null,
      priceCents: null,
      condition: null,
      photoCount: 0,
      firstUsed: null,
      lastUsed: null,
    }),
    journal: [],
    shutter: null,
  },
];

describe("export de l'inventaire", () => {
  it("séparateur du CSV selon la langue", () => {
    expect(csvDelimiter("fr")).toBe(";");
    expect(csvDelimiter("en")).toBe(",");
  });

  it("CSV : valeurs brutes pour le tableur", () => {
    const csv = inventoryCsv(t, data, "fr");
    expect(csv.headers).toHaveLength(csv.rows[0].length);
    const row = Object.fromEntries(csv.headers.map((h, i) => [h, csv.rows[0][i]]));
    expect(row).toMatchObject({
      Type: "Boîtier",
      Prix: "1899,00",
      Devise: "EUR",
      "Date d'achat": "2024-03-12",
      Photos: "1254",
      Déclenchements: "12480",
      "Dernier entretien": "2026-01-10",
      État: "Très bon",
    });
    // Un accessoire saisi à la main : pas de photos, pas de prix.
    expect(csv.rows[1][0]).toBe("Trépied");
    expect(csv.rows[1][7]).toBe("");
    expect(csv.rows[1][11]).toBe("");
  });

  it("PDF : tableau, fiches avec les seuls champs renseignés, journal", () => {
    const doc = inventoryPdf(t, data, "fr", "Photos");
    expect(doc.title).toBe("Inventaire du matériel");
    expect(doc.subtitle).toMatch(/^Photos · .* · 2 appareils$/);
    expect(doc.footer).toBe("Page {page} sur {pages}");
    expect(doc.table.rows[0][1]).toBe("Nikon Z 6");
    expect(doc.table.rows[0][4]).toMatch(/1\s899,00\s€/);
    expect(doc.table.rows[1][1]).toBe("Trépied");

    const [nikon, tripod] = doc.sections;
    const labels = nikon.fields.map(([l]) => l);
    expect(labels).toContain("Déclenchements");
    expect(labels).not.toContain("Garantie jusqu'au");
    expect(nikon.fields.find(([l]) => l === "Achat")?.[1]).toBe("12 mars 2024, Darty");
    expect(nikon.journal?.rows[0]).toEqual([
      "10 janv. 2026",
      "Nettoyage du capteur",
      expect.stringMatching(/45,00\s€/),
      "tous les 6 mois",
      "",
    ]);
    expect(tripod.fields).toEqual([]);
    expect(tripod.journal).toBeNull();
  });
});
