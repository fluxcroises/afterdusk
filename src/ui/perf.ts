/**
 * Mesure du passage d'une photo à la suivante dans la visionneuse (critère :
 * moins de 100 ms). Les mesures restent en mémoire ; l'affichage n'est compilé
 * que pour les essais (`VITE_PERF_OVERLAY=1`).
 */

const samples: number[] = [];
const listeners = new Set<() => void>();
const MAX = 200;

/** Enregistre le temps écoulé depuis la navigation, si elle est récente. */
export function recordTransition(navigatedAt: number): void {
  if (navigatedAt <= 0) return;
  const elapsed = performance.now() - navigatedAt;
  if (elapsed > 5000) return;
  samples.push(elapsed);
  if (samples.length > MAX) samples.shift();
  for (const l of listeners) l();
}

export function transitionStats(): { count: number; last: number; p95: number; max: number } {
  if (samples.length === 0) return { count: 0, last: 0, p95: 0, max: 0 };
  const sorted = [...samples].sort((a, b) => a - b);
  return {
    count: samples.length,
    last: samples[samples.length - 1] ?? 0,
    p95: sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * 0.95))] ?? 0,
    max: sorted[sorted.length - 1] ?? 0,
  };
}

export function onTransition(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export const perfOverlayEnabled = import.meta.env.VITE_PERF_OVERLAY === "1";
