/** Mise en forme des valeurs photographiques selon la langue. */

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
