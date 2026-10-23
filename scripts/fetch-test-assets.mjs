#!/usr/bin/env node
// Télécharge les échantillons de test-assets/manifest.json (RAW, portraits libres)
// et vérifie leur SHA-256.
//
//   node scripts/fetch-test-assets.mjs           télécharge les fichiers absents ou altérés
//   node scripts/fetch-test-assets.mjs --update  écrit dans le manifeste les sommes vides

import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import { readFile, rename, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const dir = join(dirname(fileURLToPath(import.meta.url)), "..", "test-assets");
const manifestPath = join(dir, "manifest.json");
const update = process.argv.includes("--update");

const sha256 = (buf) => createHash("sha256").update(buf).digest("hex");

const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
let failed = false;
let changed = false;

for (const file of manifest.files) {
  const target = join(dir, file.name);
  if (existsSync(target) && file.sha256 && sha256(await readFile(target)) === file.sha256) {
    console.log(`ok       ${file.name}`);
    continue;
  }

  process.stdout.write(`télécharge ${file.name}… `);
  // Wikimedia refuse les requêtes sans User-Agent identifiable.
  const res = await fetch(file.url, {
    headers: { "User-Agent": "Afterdusk-tests/1.0 (https://github.com/fluxcroises/afterdusk)" },
  });
  if (!res.ok) {
    console.log(`échec HTTP ${res.status}`);
    failed = true;
    continue;
  }
  const buf = Buffer.from(await res.arrayBuffer());
  const hash = sha256(buf);

  if (!file.sha256) {
    if (!update) {
      console.log("somme absente du manifeste (relancer avec --update)");
      failed = true;
      continue;
    }
    file.sha256 = hash;
    changed = true;
  } else if (hash !== file.sha256) {
    console.log(`somme incorrecte (${hash})`);
    failed = true;
    continue;
  }

  await writeFile(`${target}.part`, buf);
  await rename(`${target}.part`, target);
  console.log(`${(buf.length / 1e6).toFixed(1)} Mo`);
}

if (changed) {
  await writeFile(manifestPath, JSON.stringify(manifest, null, 2) + "\n");
  console.log("manifeste mis à jour");
}
process.exit(failed ? 1 : 0);
