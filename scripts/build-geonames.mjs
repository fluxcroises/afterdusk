#!/usr/bin/env node
// Construit le fichier de géocodage hors ligne (décision 0008) depuis les
// exports GeoNames (CC BY 4.0) téléchargés à la main :
//
//   node scripts/build-geonames.mjs <dossier>
//
// <dossier> contient cities1000.txt (de cities1000.zip), admin1CodesASCII.txt
// et countryInfo.txt de https://download.geonames.org/export/dump/.
// Écrit src-tauri/assets/geo/places.bin.z (zlib) et SOURCE.md.
//
// Format, petit-boutiste, avant compression :
//   « ADGEO1 »
//   u32 pays, puis par pays : 2 octets ASCII (code ISO), u8 longueur, nom anglais
//   u32 régions, puis : u16 pays, u8 longueur, nom
//   u32 fuseaux, puis : u8 longueur, nom IANA
//   u32 lieux, puis : i32 latitude et i32 longitude (1e-5 degré), u32 population,
//     u8 pays, u16 région (0xFFFF : aucune), u16 fuseau, u8 longueur, nom

import { deflateSync } from "node:zlib";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const source = process.argv[2];
if (!source) {
  console.error("usage : node scripts/build-geonames.mjs <dossier GeoNames>");
  process.exit(1);
}
const out = join(dirname(fileURLToPath(import.meta.url)), "..", "src-tauri", "assets", "geo");

const lines = async (name) =>
  (await readFile(join(source, name), "utf8"))
    .split("\n")
    .filter((l) => l.trim() !== "" && !l.startsWith("#"))
    .map((l) => l.split("\t"));

const countryNames = new Map((await lines("countryInfo.txt")).map((c) => [c[0], c[4]]));
const regionNames = new Map((await lines("admin1CodesASCII.txt")).map((r) => [r[0], r[1]]));

const countries = [];
const countryIndex = new Map();
const regions = [];
const regionIndex = new Map();
const zones = [];
const zoneIndex = new Map();
const index = (map, list, key, make) => {
  if (!map.has(key)) {
    map.set(key, list.length);
    list.push(make());
  }
  return map.get(key);
};

// Quartiers et arrondissements (« Marseille 02 »), lieux disparus ou
// abandonnés : jamais le nom d'une ville.
const SKIPPED = new Set(["PPLX", "PPLA5", "PPLH", "PPLQ", "PPLW", "PPLCH"]);

let places = [];
for (const c of await lines("cities1000.txt")) {
  const [, name, , , lat, lon, , feature, cc, , admin1, , , , population, , , tz] = c;
  if (!name || !cc || cc.length !== 2 || SKIPPED.has(feature)) continue;
  const country = index(countryIndex, countries, cc, () => cc);
  const key = `${cc}.${admin1}`;
  const region = regionNames.has(key)
    ? index(regionIndex, regions, key, () => [country, regionNames.get(key)])
    : 0xffff;
  const zone = index(zoneIndex, zones, tz || "", () => tz || "");
  places.push({
    lat: Math.round(Number(lat) * 1e5),
    lon: Math.round(Number(lon) * 1e5),
    population: Math.min(Number(population) || 0, 0xffffffff),
    country,
    region,
    zone,
    name,
  });
}
// Arrondissements nommés comme leur ville, suivis d'un numéro (« Paris 16
// Passy », « Lyon 03 ») : retirés quand une ville plus peuplée de ce nom est
// à moins de 20 km.
const byName = new Map();
for (const p of places) {
  if (!byName.has(p.name)) byName.set(p.name, []);
  byName.get(p.name).push(p);
}
const near = (a, b) => Math.abs(a.lat - b.lat) < 18000 && Math.abs(a.lon - b.lon) < 27000;
const districts = new Set(
  places.filter((p) => {
    const m = /^(.+?) \d/.exec(p.name);
    const parents = m && byName.get(m[1]);
    return parents?.some((c) => c.population > p.population && near(c, p));
  }),
);
places = places.filter((p) => !districts.has(p));

if (countries.length > 255 || regions.length >= 0xffff || zones.length > 0xffff) {
  throw new Error("trop d'entrées pour le format");
}

const chunks = [];
const bytes = (b) => chunks.push(Buffer.from(b));
const u8 = (v) => bytes([v]);
const u16 = (v) => {
  const b = Buffer.alloc(2);
  b.writeUInt16LE(v);
  chunks.push(b);
};
const u32 = (v) => {
  const b = Buffer.alloc(4);
  b.writeUInt32LE(v);
  chunks.push(b);
};
const i32 = (v) => {
  const b = Buffer.alloc(4);
  b.writeInt32LE(v);
  chunks.push(b);
};
/** Texte UTF-8 précédé de sa longueur, coupé à 255 octets sur un caractère entier. */
const text = (s) => {
  let b = Buffer.from(s, "utf8");
  while (b.length > 255) {
    s = s.slice(0, -1);
    b = Buffer.from(s, "utf8");
  }
  u8(b.length);
  chunks.push(b);
};

bytes("ADGEO1");
u32(countries.length);
for (const cc of countries) {
  bytes(cc);
  text(countryNames.get(cc) ?? cc);
}
u32(regions.length);
for (const [country, name] of regions) {
  u16(country);
  text(name);
}
u32(zones.length);
for (const z of zones) text(z);
u32(places.length);
for (const p of places) {
  i32(p.lat);
  i32(p.lon);
  u32(p.population);
  u8(p.country);
  u16(p.region);
  u16(p.zone);
  text(p.name);
}

const raw = Buffer.concat(chunks);
const packed = deflateSync(raw, { level: 9 });
await mkdir(out, { recursive: true });
await writeFile(join(out, "places.bin.z"), packed);
const date = new Date().toISOString().slice(0, 10);
await writeFile(
  join(out, "SOURCE.md"),
  `# Données de géocodage

\`places.bin.z\` est construit par \`scripts/build-geonames.mjs\` depuis les exports
[GeoNames](https://www.geonames.org/) \`cities1000\`, \`admin1CodesASCII\` et
\`countryInfo\`, téléchargés le ${date}.

Licence des données : [Creative Commons Attribution 4.0](https://creativecommons.org/licenses/by/4.0/),
GeoNames. ${places.length} lieux de plus de 1000 habitants (sans quartiers ni lieux
disparus), ${countries.length} pays,
${regions.length} régions.
`,
);
console.log(
  `${places.length} lieux, ${countries.length} pays, ${regions.length} régions, ${zones.length} fuseaux : ` +
    `${(raw.length / 1e6).toFixed(1)} Mo, ${(packed.length / 1e6).toFixed(1)} Mo compressés`,
);
