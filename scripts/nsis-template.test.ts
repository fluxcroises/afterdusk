// @vitest-environment node
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

// Le modèle NSIS de Tauri est embarqué en texte dans le binaire de la CLI
// installée : notre copie doit lui être identique, au dossier d'installation près.
const root = join(__dirname, "..");
const cliDir = join(root, "node_modules", "@tauri-apps");

function embeddedTemplate(): string | null {
  const pkg = readdirSync(cliDir).find((d) => d.startsWith("cli-"));
  if (!pkg) return null;
  const file = readdirSync(join(cliDir, pkg)).find((f) => f.endsWith(".node"));
  if (!file) return null;
  const bin = readFileSync(join(cliDir, pkg, file));
  const marker = Buffer.from('StrCpy $INSTDIR "$PROGRAMFILES64\\${PRODUCTNAME}"');
  const inside = bin.indexOf(marker);
  if (inside < 0) return null;
  const start = bin.lastIndexOf(Buffer.from("Unicode true"), inside);
  // Le modèle finit par la fonction du raccourci du bureau ; selon le système,
  // le texte qui le suit dans le binaire le touche sans séparateur.
  const tail = Buffer.from('!insertmacro SetLnkAppUserModelId "$DESKTOP\\${PRODUCTNAME}.lnk"');
  const last = bin.indexOf(tail, inside);
  if (last < 0) return null;
  const end = bin.indexOf(Buffer.from("FunctionEnd"), last) + "FunctionEnd".length;
  return bin.subarray(start, end).toString("utf8").replace(/\r\n/g, "\n");
}

describe("modèle NSIS de l'installateur Windows", () => {
  it("ne diffère du modèle de Tauri que par le dossier de l'éditeur", () => {
    const upstream = embeddedTemplate();
    expect(upstream, "modèle introuvable dans la CLI de Tauri").not.toBeNull();
    const ours = readFileSync(join(root, "src-tauri", "windows", "installer.nsi"), "utf8")
      .replace(/\r\n/g, "\n")
      .replace(/^(;.*\n)+/, "")
      .trimEnd();
    const expected = (upstream ?? "").replace(
      /(\$PROGRAMFILES(?:64)?)\\\$\{PRODUCTNAME\}/g,
      "$1\\${MANUFACTURER}\\${PRODUCTNAME}",
    );
    expect(ours).toBe(expected);
  });
});
