import { useEffect, useState } from "react";
import { onTransition, perfOverlayEnabled, transitionStats } from "../ui/perf";

/** Mesures de la visionneuse, affichées seulement dans les compilations d'essai. */
export function PerfOverlay() {
  const [stats, setStats] = useState(transitionStats);
  useEffect(() => onTransition(() => setStats(transitionStats())), []);
  if (!perfOverlayEnabled || stats.count === 0) return null;
  const ms = (v: number) => `${v.toFixed(0)} ms`;
  return (
    // Texte technique, hors i18n : jamais présent dans une version distribuée.
    <div className="perf-overlay">
      n={stats.count} dernier={ms(stats.last)} p95={ms(stats.p95)} max={ms(stats.max)}
    </div>
  );
}
