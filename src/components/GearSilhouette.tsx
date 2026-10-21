import { Backpack, BatteryFull, Circle, type LucideIcon, Package, Zap } from "lucide-react";
import type { GearKind, GearStyle } from "../store/gear";

/**
 * Silhouettes dessinées pour Afterdusk (décision 0008) : ni photo ni logo de
 * fabricant. Traits simples, couleur du texte, 64 × 40.
 */
const SHAPES: Record<GearStyle | "tripod", string[]> = {
  // Hybride : boîtier plat, viseur décalé à gauche, poignée, objectif court.
  mirrorless: [
    "M6 12h10l2-4h10l2 4h26a2 2 0 0 1 2 2v20a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V14a2 2 0 0 1 2-2z",
    "M48 12v24",
    "M30 24m-9 0a9 9 0 1 0 18 0a9 9 0 1 0-18 0",
    "M30 24m-5 0a5 5 0 1 0 10 0a5 5 0 1 0-10 0",
  ],
  // Reflex : prisme au centre, poignée marquée, gros objectif.
  dslr: [
    "M6 14h14l4-8h16l4 8h14a2 2 0 0 1 2 2v18a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V16a2 2 0 0 1 2-2z",
    "M50 14v22",
    "M32 25m-10 0a10 10 0 1 0 20 0a10 10 0 1 0-20 0",
    "M32 25m-6 0a6 6 0 1 0 12 0a6 6 0 1 0-12 0",
  ],
  // Compact : fin, objectif central, flash.
  compact: [
    "M10 12h44a2 2 0 0 1 2 2v16a2 2 0 0 1-2 2H10a2 2 0 0 1-2-2V14a2 2 0 0 1 2-2z",
    "M32 22m-6 0a6 6 0 1 0 12 0a6 6 0 1 0-12 0",
    "M44 15h7v4h-7z",
  ],
  // Focale fixe : cylindre court, vu de côté.
  prime: ["M14 12h26l4 3v10l-4 3H14z", "M44 15h6v10h-6", "M22 12v16", "M30 12v16"],
  // Zoom : cylindre long, bagues.
  zoom: [
    "M6 12h44l4 3v10l-4 3H6z",
    "M54 15h5v10h-5",
    "M16 12v16",
    "M20 12v16",
    "M34 12v16",
    "M38 12v16",
  ],
  // Trépied : rotule et trois pieds.
  tripod: ["M26 4h12v5H26z", "M32 9v6", "M32 15L18 38", "M32 15L32 38", "M32 15L46 38"],
};

const ICONS: Partial<Record<GearKind, LucideIcon>> = {
  // Flash : l'éclair dit littéralement ce qu'est l'appareil.
  flash: Zap,
  filter: Circle,
  bag: Backpack,
  battery: BatteryFull,
  other: Package,
};

function shapeOf(kind: GearKind, style: GearStyle | null): GearStyle | "tripod" | null {
  if (kind === "tripod") return "tripod";
  if (kind === "body") return style === "dslr" || style === "compact" ? style : "mirrorless";
  if (kind === "lens") return style === "zoom" ? "zoom" : "prime";
  return null;
}

export function GearSilhouette({ kind, style }: { kind: GearKind; style: GearStyle | null }) {
  const shape = shapeOf(kind, style);
  if (!shape) {
    const Icon = ICONS[kind] ?? Package;
    return <Icon className="gear-silhouette" aria-hidden="true" strokeWidth={1.25} />;
  }
  return (
    <svg
      className="gear-silhouette"
      viewBox="0 0 64 40"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.5}
      strokeLinejoin="round"
      strokeLinecap="round"
      aria-hidden="true"
    >
      {SHAPES[shape].map((d) => (
        <path key={d} d={d} />
      ))}
    </svg>
  );
}
