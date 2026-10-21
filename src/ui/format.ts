/** Mise en forme des valeurs photographiques selon la langue. */
import type { AlbumSummary } from "../store/library";

export function formatShutter(seconds: number | null): string | null {
  if (seconds === null || seconds <= 0) return null;
  if (seconds >= 0.4) return `${Number(seconds.toFixed(1))} s`;
  return `1/${Math.round(1 / seconds)} s`;
}

export function formatAperture(aperture: number | null, language: string): string | null {
  if (aperture === null || aperture <= 0) return null;
  return `f / ${new Intl.NumberFormat(language, { maximumFractionDigits: 1 }).format(aperture)}`;
}

export function formatFocal(focal: number | null, language: string): string | null {
  if (focal === null || focal <= 0) return null;
  return `${new Intl.NumberFormat(language, { maximumFractionDigits: 1 }).format(focal)} mm`;
}

export function formatIso(iso: number | null): string | null {
  return iso === null || iso <= 0 ? null : `ISO ${Math.round(iso)}`;
}

export function formatCompensation(ev: number | null, language: string): string | null {
  if (ev === null) return null;
  const value = new Intl.NumberFormat(language, {
    maximumFractionDigits: 1,
    signDisplay: "exceptZero",
  }).format(ev);
  return `${value} EV`;
}

/** `AAAA-MM-JJTHH:MM:SS` (heure du boîtier) en date et heure lisibles. */
export function formatCaptureTime(value: string | null, language: string): string | null {
  if (!value) return null;
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat(language, { dateStyle: "medium", timeStyle: "medium" }).format(
    date,
  );
}

/** `AAAA-MM-JJ` en date lisible, midi pour rester sur le même jour. */
export function formatDay(value: string, language: string): string {
  const date = new Date(`${value}T12:00:00`);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat(language, { dateStyle: "medium" }).format(date);
}

/** Montant en centimes dans sa devise. */
export function formatMoney(cents: number, currency: string | null, language: string): string {
  return new Intl.NumberFormat(language, { style: "currency", currency: currency ?? "EUR" }).format(
    cents / 100,
  );
}

/** Dates d'un album : un jour, ou du premier au dernier. */
export function albumDates(
  album: Pick<AlbumSummary, "startDate" | "endDate">,
  language: string,
): string | null {
  if (!album.startDate) return null;
  const format = new Intl.DateTimeFormat(language, { dateStyle: "medium" });
  // Midi : le jour reste le même quel que soit le fuseau.
  const day = (d: string) => new Date(`${d}T12:00:00`);
  const start = day(album.startDate);
  if (!album.endDate || album.endDate === album.startDate) return format.format(start);
  return `${format.format(start)} – ${format.format(day(album.endDate))}`;
}

/** Degrés sans signe, quatre décimales (une dizaine de mètres) : « 49,2869° ». */
export function formatDegrees(value: number, language: string): string {
  return `${Math.abs(value).toLocaleString(language, {
    minimumFractionDigits: 4,
    maximumFractionDigits: 4,
  })}°`;
}

/** « +02:00 » d'un décalage en minutes. */
export function formatOffset(minutes: number): string {
  const sign = minutes < 0 ? "-" : "+";
  const abs = Math.abs(minutes);
  const h = String(Math.floor(abs / 60)).padStart(2, "0");
  const m = String(abs % 60).padStart(2, "0");
  return `${sign}${h}:${m}`;
}

/** « Cabourg, France » : la ville, et le pays dans la langue de l'interface. */
export function placeName(
  city: string | null,
  country: string | null,
  language: string,
): string | undefined {
  if (!city) return undefined;
  if (!country) return city;
  let name = country;
  try {
    name = new Intl.DisplayNames([language], { type: "region" }).of(country) ?? country;
  } catch {
    // Code inconnu du navigateur : le code ISO.
  }
  return `${city}, ${name}`;
}

/** Correction d'heure : « −6 h », « +5 h 30 ». */
export function formatShift(minutes: number, hours: string): string {
  const sign = minutes < 0 ? "−" : "+";
  const abs = Math.abs(minutes);
  const h = Math.floor(abs / 60);
  const m = abs % 60;
  return m === 0 ? `${sign}${h} ${hours}` : `${sign}${h} ${hours} ${String(m).padStart(2, "0")}`;
}
