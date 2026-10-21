import { useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

export interface Bar {
  label: string;
  count: number;
}

/** Largeur avant la première mesure (et dans les tests, sans mise en page). */
const DEFAULT_WIDTH = 280;
const HEIGHT = 110;
const AXIS = 18;
const GAP = 2;
/** Peu de valeurs : des barres étroites, centrées, plutôt qu'un aplat. */
const MAX_BAR = 32;
/** Place d'une étiquette : au-delà, une sur deux, sur trois… */
const LABEL_WIDTH = 56;

/** Largeur réelle de l'élément : le graphique se dessine à sa taille, sans
 * mise à l'échelle qui rapetisserait le texte. */
function useWidth() {
  const ref = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(DEFAULT_WIDTH);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(([entry]) => {
      const w = Math.floor(entry.contentRect.width);
      if (w > 0) setWidth(w);
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, []);
  return [ref, width] as const;
}

/** Haut arrondi de 4 px, pied posé sur l'axe. */
function barPath(x: number, w: number, h: number) {
  const base = HEIGHT - AXIS;
  const r = Math.min(4, w / 2, h);
  return `M${x},${base}V${base - h + r}Q${x},${base - h} ${x + r},${base - h}H${x + w - r}Q${x + w},${base - h} ${x + w},${base - h + r}V${base}Z`;
}

/**
 * Histogramme d'une seule série. Le survol donne la valeur exacte ; sans
 * survol, la légende indique la barre la plus haute. Un tableau masqué
 * reprend les valeurs pour les lecteurs d'écran.
 */
export function BarChart({ title, bars }: { title: string; bars: Bar[] }) {
  const { t, i18n } = useTranslation();
  const [hover, setHover] = useState<number | null>(null);
  const [ref, width] = useWidth();
  if (bars.length === 0) return null;
  const max = Math.max(...bars.map((b) => b.count));
  const top = bars.findIndex((b) => b.count === max);
  const band = width / bars.length;
  const every = Math.ceil(bars.length / Math.max(1, Math.floor(width / LABEL_WIDTH)));
  const shown = bars[hover ?? top];
  const number = new Intl.NumberFormat(i18n.language);
  return (
    <figure className="bar-chart">
      <figcaption className="bar-chart-head">
        <span className="bar-chart-title">{title}</span>
        <span className="bar-chart-value">
          {shown.label} ·{" "}
          {t("gear.stats.photoCount", { count: shown.count, n: number.format(shown.count) })}
        </span>
      </figcaption>
      <div ref={ref}>
        <svg width={width} height={HEIGHT} aria-hidden="true" onMouseLeave={() => setHover(null)}>
          <line
            className="bar-chart-axis"
            x1={0}
            x2={width}
            y1={HEIGHT - AXIS + 0.5}
            y2={HEIGHT - AXIS + 0.5}
          />
          {bars.map((b, i) => {
            const h = Math.max(1, (b.count / max) * (HEIGHT - AXIS - 4));
            const w = Math.max(1, Math.min(band - GAP, MAX_BAR));
            const x = i * band + (band - w) / 2;
            return (
              <g key={b.label} onMouseEnter={() => setHover(i)}>
                <rect className="bar-chart-hit" x={i * band} y={0} width={band} height={HEIGHT} />
                <path
                  className={i === hover ? "bar-chart-bar is-hover" : "bar-chart-bar"}
                  d={barPath(x, w, h)}
                />
                {i % every === 0 && (
                  <text className="bar-chart-label" x={x + w / 2} y={HEIGHT - 4}>
                    {b.label}
                  </text>
                )}
              </g>
            );
          })}
        </svg>
      </div>
      <table className="visually-hidden">
        <tbody>
          {bars.map((b) => (
            <tr key={b.label}>
              <th scope="row">{b.label}</th>
              <td>{b.count}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </figure>
  );
}
