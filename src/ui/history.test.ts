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

  it("en anglais", () => {
    const t = i18next.getFixedT("en");
    expect(describeStep(step("light.exposure", { light: { exposure: -1.5 } }), t, "en")).toBe(
      "Exposure -1.50 EV",
    );
  });
});
