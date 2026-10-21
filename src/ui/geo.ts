/** Zone tracée sur la carte : ouest, sud, est, nord, en degrés. */
export type Area = [number, number, number, number];

/** Zone en millionièmes de degré, comme l'attend le filtre du cœur. */
export function toMicro(area: Area): [number, number, number, number] {
  return area.map((v) => Math.round(v * 1e6)) as [number, number, number, number];
}

/** Longitude ramenée entre -180 et 180 (la carte fait le tour du monde). */
export function wrapLongitude(lon: number): number {
  return ((((lon + 180) % 360) + 360) % 360) - 180;
}

/** Zone de deux coins quelconques ; l'ouest peut dépasser l'est (180°). */
export function areaOf(a: [number, number], b: [number, number]): Area {
  const [west, east] = a[0] <= b[0] ? [a[0], b[0]] : [b[0], a[0]];
  return [wrapLongitude(west), Math.min(a[1], b[1]), wrapLongitude(east), Math.max(a[1], b[1])];
}

/** Le point est-il dans la zone, y compris à cheval sur 180° ? */
export function inArea(lon: number, lat: number, [west, south, east, north]: Area): boolean {
  if (lat < south || lat > north) return false;
  return west <= east ? lon >= west && lon <= east : lon >= west || lon <= east;
}
