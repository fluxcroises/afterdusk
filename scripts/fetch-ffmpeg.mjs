#!/usr/bin/env node
// Récupère FFmpeg 9.0.2 (décision 0012) : ffmpeg et ffprobe, archives
// vérifiées par SHA-256, extraits dans src-tauri/resources/ffmpeg/, livrés
// avec Afterdusk. Rien n'est téléchargé à l'exécution de l'application.
//
//   Windows : version GPL en bibliothèques partagées de BtbN/FFmpeg-Builds
//             (publication mensuelle, conservée).
//   macOS   : versions statiques de Martin Riedl pour Apple Silicon et
//             Intel, réunies en binaires universels par lipo.
//
//   node scripts/fetch-ffmpeg.mjs   télécharge si la version attendue manque

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import {
  chmod,
  copyFile,
  mkdir,
  mkdtemp,
  readdir,
  readFile,
  rm,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const VERSION = "9.0.2";

// Empreintes publiées par GitHub (BtbN) et par ffmpeg.martin-riedl.de (.sha256),
// relevées le 2026-10-22.
const WINDOWS = {
  url: "https://github.com/BtbN/FFmpeg-Builds/releases/download/autobuild-2026-09-30-13-08/ffmpeg-n9.0.2-17-g2a571b6068-win64-gpl-shared-9.0.zip",
  sha256: "3da6c7b60bb9ccd73ec5b5e815ba804a0879eb362ba0e3beebce50174c022696",
};
const MAC = "https://ffmpeg.martin-riedl.de/download/macos";
const MACOS = [
  {
    arch: "arm64",
    tool: "ffmpeg",
    url: `${MAC}/arm64/1789931890_9.0.2/ffmpeg.zip`,
    sha256: "c8ed4c4e6978a03c485edbfe4e0a5dc2380f8a30bba5150531b31b094492d924",
  },
  {
    arch: "arm64",
    tool: "ffprobe",
    url: `${MAC}/arm64/1789931890_9.0.2/ffprobe.zip`,
    sha256: "fcbe839537485eaee7a7a8bc5cbc0f90d53617e80943e8a5b2e31cb851197ea6",
  },
  {
    arch: "x86_64",
    tool: "ffmpeg",
    url: `${MAC}/amd64/1789931006_9.0.2/ffmpeg.zip`,
    sha256: "7c6b4125b191cbf773832dc51f424cf2b6bb7da43007d1e066f95909e47cacd4",
  },
  {
    arch: "x86_64",
    tool: "ffprobe",
    url: `${MAC}/amd64/1789931006_9.0.2/ffprobe.zip`,
    sha256: "2322438ed2f6319a691291b247d09c69dcaa3a982460d1f269a7e1af335cfdfd",
  },
];

const NOTICE = `FFmpeg ${VERSION}, livré avec Afterdusk (décision 0012).

FFmpeg est distribué sous licence GNU GPL version 2 ou ultérieure dans cette
construction (bibliothèques GPL incluses, dont x264). Afterdusk l'utilise
comme programme séparé (ffmpeg, ffprobe).

Code source correspondant : https://ffmpeg.org/releases/ffmpeg-${VERSION}.tar.xz
Mentions légales : https://ffmpeg.org/legal.html
Constructions : Windows, https://github.com/BtbN/FFmpeg-Builds ;
macOS, https://ffmpeg.martin-riedl.de (scripts : https://git.martin-riedl.de/ffmpeg/build-script).
`;

const out = join(dirname(fileURLToPath(import.meta.url)), "..", "src-tauri", "resources", "ffmpeg");
const exe = process.platform === "win32" ? ".exe" : "";
const marker = join(out, "VERSION");
const stamp = `${VERSION} ${process.platform === "win32" ? WINDOWS.sha256 : MACOS.map((m) => m.sha256).join(" ")}\n`;

if (process.platform !== "win32" && process.platform !== "darwin") {
  console.log(`FFmpeg : pas d'archive prévue pour ${process.platform}`);
  process.exit(1);
}
if (existsSync(join(out, `ffprobe${exe}`)) && existsSync(marker)) {
  if ((await readFile(marker, "utf8")) === stamp) {
    console.log(`ok       FFmpeg ${VERSION}`);
    process.exit(0);
  }
}

async function download(url, sha256) {
  const name = url.split("/").slice(-3).join("/");
  process.stdout.write(`télécharge ${name}… `);
  const res = await fetch(url);
  if (!res.ok) {
    console.log(`échec HTTP ${res.status}`);
    process.exit(1);
  }
  const buf = Buffer.from(await res.arrayBuffer());
  const hash = createHash("sha256").update(buf).digest("hex");
  if (hash !== sha256) {
    console.log(`somme incorrecte (${hash})`);
    process.exit(1);
  }
  console.log(`${(buf.length / 1e6).toFixed(1)} Mo`);
  return buf;
}

// bsdtar ouvre les zip : celui du système sous Windows (le tar de Git
// prendrait « C: » pour un hôte distant).
const tar =
  process.platform === "win32"
    ? join(process.env.SystemRoot ?? "C:\\Windows", "System32", "tar.exe")
    : "tar";

const work = await mkdtemp(join(tmpdir(), "ffmpeg-"));
try {
  await rm(out, { recursive: true, force: true });
  await mkdir(out, { recursive: true });
  if (process.platform === "win32") {
    await writeFile(join(work, "ffmpeg.zip"), await download(WINDOWS.url, WINDOWS.sha256));
    execFileSync(tar, ["-xf", "ffmpeg.zip"], { cwd: work });
    const root = (await readdir(work)).find((n) => n.startsWith("ffmpeg-n"));
    const bin = join(work, root, "bin");
    // ffplay n'est pas utile : ffmpeg, ffprobe et leurs bibliothèques.
    for (const file of await readdir(bin)) {
      if (file.endsWith(".dll") || file === "ffmpeg.exe" || file === "ffprobe.exe") {
        await copyFile(join(bin, file), join(out, file));
      }
    }
    await copyFile(join(work, root, "LICENSE.txt"), join(out, "LICENSE.txt"));
  } else {
    for (const m of MACOS) {
      const dir = join(work, m.arch, m.tool);
      await mkdir(dir, { recursive: true });
      await writeFile(join(dir, "a.zip"), await download(m.url, m.sha256));
      execFileSync(tar, ["-xf", "a.zip"], { cwd: dir });
    }
    for (const tool of ["ffmpeg", "ffprobe"]) {
      execFileSync("lipo", [
        "-create",
        join(work, "arm64", tool, tool),
        join(work, "x86_64", tool, tool),
        "-output",
        join(out, tool),
      ]);
      await chmod(join(out, tool), 0o755);
    }
  }
  await writeFile(join(out, "NOTICE.txt"), NOTICE);
  await writeFile(marker, stamp);
  console.log(`extrait  ffmpeg et ffprobe dans src-tauri/resources/ffmpeg`);
} finally {
  await rm(work, { recursive: true, force: true });
}
