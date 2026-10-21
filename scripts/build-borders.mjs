#!/usr/bin/env node
// Fond de carte hors ligne (décision 0008) : frontières des pays Natural Earth
// au 1:50m (domaine public), téléchargées à la main :
//
//   node scripts/build-borders.mjs <ne_50m_admin_0_countries.geojson>
//
// Ne garde que la géométrie et le code ISO du pays, coordonnées arrondies à
// 0,001° (une centaine de mètres) sans points répétés. Écrit
// src/assets/geo/countries.json et SOURCE.md.

import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const source = process.argv[2];
if (!source) {
  console.error("usage : node scripts/build-borders.mjs <ne_50m_admin_0_countries.geojson>");
  process.exit(1);
}
const out = join(dirname(fileURLToPath(import.meta.url)), "..", "src", "assets", "geo");

const round = (v) => Math.round(v * 1000) / 1000;

/** Anneau arrondi, sans points consécutifs identiques, encore fermé. */
function ring(points) {
  const kept = [];
  for (const [x, y] of points) {
    const p = [round(x), round(y)];
    const last = kept[kept.length - 1];
    if (!last || last[0] !== p[0] || last[1] !== p[1]) kept.push(p);
  }
  return kept.length >= 4 ? kept : null;
}

const polygon = (rings) => {
  const kept = rings.map(ring).filter(Boolean);
  return kept.length > 0 ? kept : null;
};

const data = JSON.parse(await readFile(source, "utf8"));
const features = [];
for (const f of data.features) {
  const g = f.geometry;
  let coordinates = null;
  if (g?.type === "Polygon") coordinates = polygon(g.coordinates);
  else if (g?.type === "MultiPolygon") {
    const parts = g.coordinates.map(polygon).filter(Boolean);
    coordinates = parts.length > 0 ? parts : null;
  }
  if (!coordinates) continue;
  const iso = f.properties.ISO_A2_EH !== "-99" ? f.properties.ISO_A2_EH : f.properties.ADM0_A3;
  features.push({
    type: "Feature",
    properties: { iso },
    geometry: { type: g.type, coordinates },
  });
}

const json = JSON.stringify({ type: "FeatureCollection", features });
await mkdir(out, { recursive: true });
await writeFile(join(out, "countries.json"), json);
await writeFile(
  join(out, "SOURCE.md"),
  `# Fond de carte hors ligne

\`countries.json\` est construit par \`scripts/build-borders.mjs\` depuis
[Natural Earth](https://www.naturalearthdata.com/) \`ne_50m_admin_0_countries\`
(1:50m, domaine public), téléchargé le ${new Date().toISOString().slice(0, 10)}.
${features.length} pays, coordonnées arrondies à 0,001°.
`,
);
console.log(`${features.length} pays : ${(json.length / 1e6).toFixed(2)} Mo`);
