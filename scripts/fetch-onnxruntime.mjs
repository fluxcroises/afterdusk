#!/usr/bin/env node
// Récupère ONNX Runtime (décision 0010) depuis la publication officielle de
// Microsoft, vérifie l'archive par SHA-256 et en extrait la bibliothèque et
// ses licences dans src-tauri/resources/onnxruntime/, livrées avec Afterdusk.
// Rien n'est téléchargé à l'exécution de l'application.
//
//   node scripts/fetch-onnxruntime.mjs   télécharge si la version attendue manque

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { copyFile, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const VERSION = "1.23.2";
const BASE = `https://github.com/microsoft/onnxruntime/releases/download/v${VERSION}`;

// Empreintes calculées sur les archives officielles le 2026-10-21 (GitHub
// n'en publie pas pour cette version).
const PLATFORMS = {
  "win32-x64": {
    archive: `onnxruntime-win-x64-${VERSION}.zip`,
    sha256: "0b38df9af21834e41e73d602d90db5cb06dbd1ca618948b8f1d66d607ac9f3cd",
    lib: "lib/onnxruntime.dll",
    target: "onnxruntime.dll",
  },
  // Bibliothèque universelle : Apple Silicon et Intel.
  darwin: {
    archive: `onnxruntime-osx-universal2-${VERSION}.tgz`,
    sha256: "49ae8e3a66ccb18d98ad3fe7f5906b6d7887df8a5edd40f49eb2b14e20885809",
    lib: `lib/libonnxruntime.${VERSION}.dylib`,
    target: "libonnxruntime.dylib",
  },
};

const platform = PLATFORMS[`${process.platform}-${process.arch}`] ?? PLATFORMS[process.platform];
if (!platform) {
  console.log(`ONNX Runtime : pas d'archive prévue pour ${process.platform}-${process.arch}`);
  process.exit(1);
}

const out = join(
  dirname(fileURLToPath(import.meta.url)),
  "..",
  "src-tauri",
  "resources",
  "onnxruntime",
);
const marker = join(out, "VERSION");
const stamp = `${VERSION} ${platform.sha256}\n`;
if (existsSync(join(out, platform.target)) && existsSync(marker)) {
  if ((await readFile(marker, "utf8")) === stamp) {
    console.log(`ok       ONNX Runtime ${VERSION}`);
    process.exit(0);
  }
}

process.stdout.write(`télécharge ${platform.archive}… `);
const res = await fetch(`${BASE}/${platform.archive}`);
if (!res.ok) {
  console.log(`échec HTTP ${res.status}`);
  process.exit(1);
}
const buf = Buffer.from(await res.arrayBuffer());
const hash = createHash("sha256").update(buf).digest("hex");
if (hash !== platform.sha256) {
  console.log(`somme incorrecte (${hash})`);
  process.exit(1);
}
console.log(`${(buf.length / 1e6).toFixed(1)} Mo`);

const work = await mkdtemp(join(tmpdir(), "onnxruntime-"));
try {
  const archive = join(work, platform.archive);
  await writeFile(archive, buf);
  // bsdtar ouvre le zip comme le tgz : celui du système sous Windows (un
  // autre tar, celui de Git, prendrait « C: » pour un hôte distant).
  const tar =
    process.platform === "win32"
      ? join(process.env.SystemRoot ?? "C:\\Windows", "System32", "tar.exe")
      : "tar";
  execFileSync(tar, ["-xf", platform.archive], { cwd: work });
  const root = join(work, platform.archive.replace(/\.(zip|tgz)$/, ""));
  await rm(out, { recursive: true, force: true });
  await mkdir(out, { recursive: true });
  await copyFile(join(root, platform.lib), join(out, platform.target));
  await copyFile(join(root, "LICENSE"), join(out, "LICENSE"));
  await copyFile(join(root, "ThirdPartyNotices.txt"), join(out, "ThirdPartyNotices.txt"));
  await writeFile(marker, stamp);
  console.log(`extrait  ${platform.target} dans src-tauri/resources/onnxruntime`);
} finally {
  await rm(work, { recursive: true, force: true });
}
