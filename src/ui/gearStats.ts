import type { Bar } from "../components/BarChart";
import type { GearStats, ValueCount } from "../store/gear";

/** Au-delà, les focales se regroupent par tranches. */
const MAX_FOCALS = 24;
/** Ouvertures et ISO : les plus fréquentes seulement, rangées par valeur. */
const MAX_VALUES = 16;

function niceStep(raw: number) {
  return [5, 10, 20, 25, 50, 100, 200].find((s) => s >= raw) ?? 500;
}

export function focalBars(focal: ValueCount[]): Bar[] {
  if (focal.length <= MAX_FOCALS) {
    return focal.map((f) => ({ label: `${f.value} mm`, count: f.count }));
  }
  const min = focal[0].value;
  const max = focal[focal.length - 1].value;
  const step = niceStep((max - min) / 16);
  const bins = new Map<number, number>();
  for (const f of focal) {
    const start = Math.floor(f.value / step) * step;
    bins.set(start, (bins.get(start) ?? 0) + f.count);
  }
  return [...bins.entries()]
    .sort(([a], [b]) => a - b)
    .map(([start, count]) => ({ label: `${start}–${start + step - 1} mm`, count }));
}

function frequent(values: ValueCount[], label: (v: number) => string): Bar[] {
  return [...values]
    .sort((a, b) => b.count - a.count)
    .slice(0, MAX_VALUES)
    .sort((a, b) => a.value - b.value)
    .map((v) => ({ label: label(v.value), count: v.count }));
}

export const apertureBars = (v: ValueCount[]) => frequent(v, (a) => `f/${a}`);
export const isoBars = (v: ValueCount[]) => frequent(v, (i) => String(i));

/** Photos par mois, mois sans photo compris ; par année au-delà de trois ans. */
export function monthBars(months: GearStats["months"], language: string): Bar[] {
  if (months.length === 0) return [];
  const index = (m: string) => Number(m.slice(0, 4)) * 12 + Number(m.slice(5, 7)) - 1;
  const first = index(months[0].month);
  const last = index(months[months.length - 1].month);
  const counts = new Map(months.map((m) => [index(m.month), m.count]));
  if (last - first >= 36) {
    const years = new Map<number, number>();
    for (const m of months) {
      const y = Number(m.month.slice(0, 4));
      years.set(y, (years.get(y) ?? 0) + m.count);
    }
    const bars: Bar[] = [];
    for (let y = Math.floor(first / 12); y <= Math.floor(last / 12); y++) {
      bars.push({ label: String(y), count: years.get(y) ?? 0 });
    }
    return bars;
  }
  const format = new Intl.DateTimeFormat(language, { month: "short", year: "numeric" });
  const bars: Bar[] = [];
  for (let i = first; i <= last; i++) {
    const date = new Date(Math.floor(i / 12), i % 12, 1);
    bars.push({ label: format.format(date), count: counts.get(i) ?? 0 });
  }
  return bars;
}
