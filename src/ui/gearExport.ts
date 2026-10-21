import { invoke } from "@tauri-apps/api/core";
import type { TFunction } from "i18next";
import type { GearItem, GearRecord, GearStats } from "../store/gear";
import type { MaintenanceEntry } from "../store/maintenance";
import { formatCaptureTime, formatDay, formatMoney } from "./format";

/** Miroir de `gear::export::Table`. */
export interface ExportTable {
  headers: string[];
  rows: string[][];
  widths?: number[];
}

/** Miroir de `gear::export::InventoryPdf`. */
export interface InventoryPdf {
  title: string;
  subtitle: string;
  table: ExportTable;
  sections: {
    gearId: number;
    title: string;
    subtitle: string;
    fields: [string, string][];
    journal: ExportTable | null;
  }[];
  footer: string;
}

/** Tout ce qu'il faut savoir d'un appareil pour l'exporter. */
export interface GearExportData {
  record: GearRecord;
  journal: MaintenanceEntry[];
  /** Déclenchements (boîtiers) : lus dans une photo, ou estimés d'après les photos. */
  shutter: { count: number; read: boolean } | null;
}

/** Matériel visible, avec fiche, journal et compteur. */
export async function collectGear(items: GearItem[]): Promise<GearExportData[]> {
  return Promise.all(
    items
      .filter((g) => !g.hidden)
      .map(async (g) => {
        const [record, journal, stats] = await Promise.all([
          invoke<GearRecord>("gear_get", { id: g.id }),
          invoke<MaintenanceEntry[]>("maintenance_list", { gearId: g.id }),
          g.kind === "body" && g.autoDetected
            ? invoke<GearStats>("gear_stats", { id: g.id })
            : Promise.resolve(null),
        ]);
        const s = stats?.shutter;
        return {
          record,
          journal,
          shutter: s ? { count: s.estimate, read: s.read !== null } : null,
        };
      }),
  );
}

/** Point-virgule là où la virgule sert de séparateur décimal, comme Excel. */
export function csvDelimiter(language: string): string {
  const decimal = new Intl.NumberFormat(language)
    .formatToParts(1.5)
    .find((p) => p.type === "decimal");
  return decimal?.value === "," ? ";" : ",";
}

function name(t: TFunction, r: GearRecord): string {
  return r.displayName.trim() || t(`gear.kinds.${r.kind}`);
}

function price(r: GearRecord, language: string): string {
  return r.priceCents === null ? "" : formatMoney(r.priceCents, r.currency, language);
}

/** Tableur : toutes les colonnes, nombres et dates bruts pour le calcul. */
export function inventoryCsv(t: TFunction, data: GearExportData[], language: string): ExportTable {
  const decimal = (cents: number) =>
    new Intl.NumberFormat(language, { minimumFractionDigits: 2, useGrouping: false }).format(
      cents / 100,
    );
  return {
    headers: [
      t("gear.fields.kind"),
      t("gear.fields.make"),
      t("gear.fields.model"),
      t("gear.fields.name"),
      t("gear.fields.serial"),
      t("gear.fields.purchaseDate"),
      t("gear.fields.purchasePlace"),
      t("gear.fields.price"),
      t("gear.fields.currency"),
      t("gear.fields.condition"),
      t("gear.fields.warrantyUntil"),
      t("gear.photos"),
      t("gear.firstUsed"),
      t("gear.lastUsed"),
      t("gear.stats.shutter"),
      t("gearExport.lastMaintenance"),
      t("gear.fields.notes"),
    ],
    rows: data.map(({ record: r, journal, shutter }) => [
      t(`gear.kinds.${r.kind}`),
      r.make,
      r.model,
      r.name ?? "",
      r.serial ?? "",
      r.purchaseDate ?? "",
      r.purchasePlace ?? "",
      r.priceCents === null ? "" : decimal(r.priceCents),
      r.priceCents === null ? "" : (r.currency ?? "EUR"),
      r.condition ? t(`gear.conditions.${r.condition}`) : "",
      r.warrantyUntil ?? "",
      r.autoDetected ? String(r.photoCount) : "",
      r.firstUsed?.slice(0, 10) ?? "",
      r.lastUsed?.slice(0, 10) ?? "",
      // Une estimation répète la colonne Photos : seul un compteur lu y figure.
      shutter?.read ? String(shutter.count) : "",
      journal[0]?.date ?? "",
      r.notes ?? "",
    ]),
  };
}

/** Document : tableau récapitulatif, puis une fiche par appareil. */
export function inventoryPdf(
  t: TFunction,
  data: GearExportData[],
  language: string,
  libraryName: string,
): InventoryPdf {
  const number = new Intl.NumberFormat(language);
  const day = (d: string | null) => (d ? formatDay(d, language) : "");
  return {
    title: t("gearExport.title"),
    subtitle: [
      libraryName,
      formatDay(new Date().toISOString().slice(0, 10), language),
      t("gear.count", { count: data.length }),
    ]
      .filter(Boolean)
      .join(" · "),
    table: {
      headers: [
        t("gear.fields.kind"),
        t("gearExport.device"),
        t("gear.fields.serial"),
        t("gear.fields.purchaseDate"),
        t("gear.fields.price"),
        t("gear.fields.condition"),
        t("gear.photos"),
        // Colonne étroite : la date de la dernière intervention suffit.
        t("maintenance.title"),
      ],
      widths: [1, 2.4, 1.6, 1.2, 1.1, 1, 0.7, 1.3],
      rows: data.map(({ record: r, journal }) => [
        t(`gear.kinds.${r.kind}`),
        name(t, r),
        r.serial ?? "",
        day(r.purchaseDate),
        price(r, language),
        r.condition ? t(`gear.conditions.${r.condition}`) : "",
        r.autoDetected ? number.format(r.photoCount) : "",
        journal[0] ? formatDay(journal[0].date, language) : "",
      ]),
    },
    sections: data.map(({ record: r, journal, shutter }) => {
      const fields: [string, string | null][] = [
        [t("gear.fields.make"), r.make || null],
        [t("gear.fields.model"), r.model || null],
        [t("gear.fields.serial"), r.serial],
        [
          t("gearExport.purchase"),
          [day(r.purchaseDate), r.purchasePlace].filter(Boolean).join(", ") || null,
        ],
        [t("gear.fields.price"), price(r, language) || null],
        [t("gear.fields.condition"), r.condition ? t(`gear.conditions.${r.condition}`) : null],
        [t("gear.fields.warrantyUntil"), r.warrantyUntil ? day(r.warrantyUntil) : null],
        [t("gear.photos"), r.autoDetected ? number.format(r.photoCount) : null],
        [t("gear.firstUsed"), formatCaptureTime(r.firstUsed, language)],
        [t("gear.lastUsed"), formatCaptureTime(r.lastUsed, language)],
        [
          t("gear.stats.shutter"),
          shutter === null
            ? null
            : shutter.read
              ? number.format(shutter.count)
              : t("gear.stats.shutterAtLeast", { n: number.format(shutter.count) }),
        ],
        [t("gear.fields.notes"), r.notes],
      ];
      return {
        gearId: r.id,
        title: name(t, r),
        subtitle: [t(`gear.kinds.${r.kind}`), r.name ? `${r.make} ${r.model}`.trim() : null]
          .filter(Boolean)
          .join(" · "),
        fields: fields.filter((f): f is [string, string] => Boolean(f[1])),
        journal:
          journal.length === 0
            ? null
            : {
                headers: [
                  t("maintenance.date"),
                  t("maintenance.kind"),
                  t("maintenance.cost"),
                  t("maintenance.reminder"),
                  t("gear.fields.notes"),
                ],
                widths: [1.2, 1.8, 1, 1.2, 2.2],
                rows: journal.map((e) => [
                  day(e.date),
                  t(`maintenance.kinds.${e.kind}`),
                  e.costCents === null ? "" : formatMoney(e.costCents, e.currency, language),
                  e.reminderMonths === null
                    ? ""
                    : t("maintenance.every", { count: e.reminderMonths }),
                  e.notes ?? "",
                ]),
              },
      };
    }),
    footer: t("gearExport.footer", { page: "{page}", pages: "{pages}" }),
  };
}
