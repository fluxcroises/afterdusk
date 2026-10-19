import type { CurvePoint } from "../store/develop";

/**
 * Courbe cubique monotone (Fritsch–Carlson) passant par les points, de 0 à
 * 255 : la même que celle du cœur (`develop::curve`), pour la dessiner. Le
 * rendu, lui, est toujours calculé par le cœur.
 */
export function curveAt(points: CurvePoint[]): (x: number) => number {
  if (points.length < 2) return (x) => x;
  const xs = points.map((p) => p[0] / 255);
  const ys = points.map((p) => p[1] / 255);
  const n = xs.length;
  const d = xs
    .slice(0, -1)
    .map((x, i) => ((ys[i + 1] ?? 0) - (ys[i] ?? 0)) / Math.max((xs[i + 1] ?? 0) - x, 1e-9));
  const m = new Array<number>(n).fill(0);
  m[0] = d[0] ?? 0;
  m[n - 1] = d[n - 2] ?? 0;
  for (let i = 1; i < n - 1; i++) {
    const a = d[i - 1] ?? 0;
    const b = d[i] ?? 0;
    m[i] = a * b <= 0 ? 0 : (a + b) / 2;
  }
  for (let i = 0; i < n - 1; i++) {
    const di = d[i] ?? 0;
    if (di === 0) {
      m[i] = 0;
      m[i + 1] = 0;
      continue;
    }
    const a = (m[i] ?? 0) / di;
    const b = (m[i + 1] ?? 0) / di;
    const h = Math.hypot(a, b);
    if (h > 3) {
      m[i] = ((3 * a) / h) * di;
      m[i + 1] = ((3 * b) / h) * di;
    }
  }
  return (x255) => {
    const t = x255 / 255;
    let v: number;
    if (t <= (xs[0] ?? 0)) v = ys[0] ?? 0;
    else if (t >= (xs[n - 1] ?? 1)) v = ys[n - 1] ?? 1;
    else {
      let i = 0;
      while (i < n - 2 && (xs[i + 1] ?? 1) <= t) i++;
      const x0 = xs[i] ?? 0;
      const h = (xs[i + 1] ?? 1) - x0;
      const s = (t - x0) / h;
      const s2 = s * s;
      const s3 = s2 * s;
      v =
        (2 * s3 - 3 * s2 + 1) * (ys[i] ?? 0) +
        (s3 - 2 * s2 + s) * h * (m[i] ?? 0) +
        (-2 * s3 + 3 * s2) * (ys[i + 1] ?? 0) +
        (s3 - s2) * h * (m[i + 1] ?? 0);
    }
    return Math.min(1, Math.max(0, v)) * 255;
  };
}
