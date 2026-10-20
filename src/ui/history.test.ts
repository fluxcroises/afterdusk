import i18next from "i18next";
import { beforeAll, describe, expect, it } from "vitest";
import fr from "../i18n/fr.json";
import en from "../i18n/en.json";
import { describeStep as describeRaw } from "./history";

/** Espaces insécables et signe moins varient selon la version d'ICU. */
const MINUS = String.fromCharCode(0x2212);
const describeStep = (...args: Parameters<typeof describeRaw>) =>
  describeRaw(...args)
    .normalize("NFKC")
    .split(MINUS)
    .join("-");

const step = (label: string, settings: object) => ({
  position: 1,
  label,
  settings: { version: 1, process: 1, ...settings },
  createdAt: "2026-10-19T20:00:00Z",
});

const presets = [{ name: "tungsten", temperature: 2850, tint: 0 }];

beforeAll(async () => {
  await i18next.init({
    lng: "fr",
    resources: { fr: { translation: fr }, en: { translation: en } },
  });
});

describe("libellés de l'historique", () => {
  it("réglage et valeur, en français", () => {
    const t = i18next.getFixedT("fr");
    expect(describeStep(step("light.exposure", { light: { exposure: 0.7 } }), t, "fr")).toBe(
      "Exposition +0,70 IL",
    );
    expect(describeStep(step("light.shadows", { light: { shadows: -25 } }), t, "fr")).toBe(
      "Ombres -25",
    );
    expect(describeStep(step("light.contrast", {}), t, "fr")).toBe("Contraste 0");
    expect(describeStep(step("light.auto", { light: { exposure: 1 } }), t, "fr")).toBe(
      "Lumière automatique",
    );
    expect(describeStep(step("light.whites.auto", { light: { whites: 12 } }), t, "fr")).toBe(
      "Blancs (auto) +12",
    );
    expect(describeStep(step("color.vibrance", { color: { vibrance: 30 } }), t, "fr")).toBe(
      "Vibrance +30",
    );
  });

  it("balance des blancs : préréglage, valeurs, automatique", () => {
    const t = i18next.getFixedT("fr");
    const tungsten = { color: { whiteBalance: { mode: "preset", preset: "tungsten" } } };
    expect(describeStep(step("color.whiteBalance", tungsten), t, "fr", presets)).toBe(
      "Balance des blancs : Tungstène",
    );
    const custom = {
      color: { whiteBalance: { mode: "custom", temperature: 5100, tint: 3 } },
    };
    expect(describeStep(step("color.whiteBalance.auto", custom), t, "fr", presets)).toBe(
      "Balance des blancs automatique : 5 100 K, teinte +3",
    );
    expect(describeStep(step("color.whiteBalance", {}), t, "fr", presets)).toBe(
      "Balance des blancs : Telle que prise",
    );
  });

  it("instantané appliqué et copie virtuelle", () => {
    const t = i18next.getFixedT("fr");
    expect(describeStep(step("snapshot:Client: v2", {}), t, "fr")).toBe("Instantané : Client: v2");
    expect(describeStep(step("version.create", {}), t, "fr")).toBe("Copie virtuelle créée");
  });

  it("courbes, couleurs par teinte et effets", () => {
    const t = i18next.getFixedT("fr");
    expect(describeStep(step("curve.blue", {}), t, "fr")).toBe("Courbe : Bleu");
    expect(
      describeStep(step("hsl.saturation.blue", { hsl: { saturation: { blue: -40 } } }), t, "fr"),
    ).toBe("Saturation : Bleu -40");
    expect(
      describeStep(
        step("effects.vignette.amount", { effects: { vignette: { amount: -30 } } }),
        t,
        "fr",
      ),
    ).toBe("Vignetage : intensité -30");
    expect(
      describeStep(
        step("detail.sharpening.radius", { detail: { sharpening: { radius: 1.4 } } }),
        t,
        "fr",
      ),
    ).toBe("Netteté : rayon 1,4");
    expect(describeStep(step("detail.noise.luminance", {}), t, "fr")).toBe(
      "Réduction du bruit : luminance 0",
    );
    // Valeur par défaut absente du JSON : on affiche le défaut.
    expect(describeStep(step("effects.grain.size", {}), t, "fr")).toBe("Grain : taille 25");
  });

  it("géométrie", () => {
    const t = i18next.getFixedT("fr");
    expect(describeStep(step("geometry.angle", { geometry: { angle: -2.4 } }), t, "fr")).toBe(
      "Redressement -2,4°",
    );
    expect(describeStep(step("geometry.auto", { geometry: { angle: 3 } }), t, "fr")).toBe(
      "Redressement automatique +3,0°",
    );
    expect(describeStep(step("geometry.aspect", { geometry: { aspect: "3:2" } }), t, "fr")).toBe(
      "Ratio 3:2",
    );
    expect(describeStep(step("geometry.crop", {}), t, "fr")).toBe("Recadrage");
  });

  it("corrections d'objectif", () => {
    const t = i18next.getFixedT("fr");
    expect(describeStep(step("lens.enabled", { lens: { enabled: false } }), t, "fr")).toBe(
      "Corrections d'objectif désactivées",
    );
    expect(describeStep(step("lens.distortion", { lens: { distortion: 60 } }), t, "fr")).toBe(
      "Distorsion de l'objectif 60 %",
    );
    expect(describeStep(step("lens.vignetting", {}), t, "fr")).toBe(
      "Vignetage de l'objectif 100 %",
    );
  });

  it("correcteur de défauts", () => {
    const t = i18next.getFixedT("fr");
    expect(describeStep(step("spots.add", {}), t, "fr")).toBe("Correction de défaut");
    expect(describeStep(step("spots.source", {}), t, "fr")).toBe("Source de correction déplacée");
    expect(describeStep(step("spots.clear", {}), t, "fr")).toBe("Corrections effacées");
  });

  it("en anglais", () => {
    const t = i18next.getFixedT("en");
    expect(describeStep(step("light.exposure", { light: { exposure: -1.5 } }), t, "en")).toBe(
      "Exposure -1.50 EV",
    );
  });
});
