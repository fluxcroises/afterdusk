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
  /** Déclenchements estimés (boîtiers). */
  shutter: number | null;
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
        return { record, journal, shutter: stats?.shutter?.estimate ?? null };
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

function lastMaintenance(t: TFunction, journal: MaintenanceEntry[], language: string): string {
  const last = journal[0];
  return last ? `${formatDay(last.date, language)} · ${t(`maintenance.kinds.${last.kind}`)}` : "";
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
      shutter === null ? "" : String(shutter),
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
        t("gearExport.lastMaintenance"),
      ],
      widths: [1, 2.6, 1.3, 1.2, 1.1, 1, 0.7, 1.9],
      rows: data.map(({ record: r, journal }) => [
        t(`gear.kinds.${r.kind}`),
        name(t, r),
        r.serial ?? "",
        day(r.purchaseDate),
        price(r, language),
        r.condition ? t(`gear.conditions.${r.condition}`) : "",
        r.autoDetected ? number.format(r.photoCount) : "",
        lastMaintenance(t, journal, language),
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
        [t("gear.stats.shutter"), shutter === null ? null : number.format(shutter)],
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
