// Fichier latest.json lu par les versions installées pour trouver leur mise à
// jour (décision 0013). Usage : node scripts/latest-json.mjs <version> <dossier>
// Le dossier contient les fichiers de la version et leurs signatures (.sig) :
//   Afterdusk_<version>_x64-setup.exe          installateur Windows
//   Afterdusk_<version>_universal.app.tar.gz   archive de mise à jour macOS
// Le paquet macOS est universel : Apple Silicon et Intel pointent sur la même archive.
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

export const REPOSITORY = "fluxcroises/afterdusk";

export function assetNames(version) {
  return {
    windows: `Afterdusk_${version}_x64-setup.exe`,
    macos: `Afterdusk_${version}_universal.app.tar.gz`,
  };
}

/** Contenu de latest.json ; `signature(nom)` rend le contenu du .sig du fichier. */
export function latestJson(version, signature, pubDate = new Date().toISOString()) {
  const names = assetNames(version);
  const url = (name) => `https://github.com/${REPOSITORY}/releases/download/v${version}/${name}`;
  const entry = (name) => ({ signature: signature(name).trim(), url: url(name) });
  const mac = entry(names.macos);
  return {
    version,
    notes: `Afterdusk ${version}`,
    pub_date: pubDate,
    platforms: {
      "windows-x86_64": entry(names.windows),
      "darwin-aarch64": mac,
      "darwin-x86_64": mac,
    },
  };
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  const [version, dir] = process.argv.slice(2);
  if (!version || !dir || !/^\d+\.\d+\.\d+$/.test(version)) {
    console.error("usage : node scripts/latest-json.mjs <version x.y.z> <dossier>");
    process.exit(1);
  }
  const json = latestJson(version, (name) => readFileSync(join(dir, `${name}.sig`), "utf8"));
  writeFileSync(join(dir, "latest.json"), `${JSON.stringify(json, null, 2)}\n`);
  console.log(`latest.json : ${Object.keys(json.platforms).join(", ")}`);
}
