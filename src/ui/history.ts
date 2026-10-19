import type { TFunction } from "i18next";
import type { DevelopSettings, HistoryStep, WhitePreset } from "../store/develop";

/** Valeur signée, à la manière des curseurs (« +0,70 », « −25 »). */
function signed(value: number, digits: number, language: string): string {
  return new Intl.NumberFormat(language, {
    signDisplay: "exceptZero",
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  }).format(value);
}

function whiteBalance(
  settings: DevelopSettings,
  t: TFunction,
  language: string,
  presets: WhitePreset[],
): string {
  const wb = settings.color?.whiteBalance;
  if (!wb || wb.mode === "asShot") return t("develop.wb.asShot");
  if (wb.mode === "preset") {
    return presets.some((p) => p.name === wb.preset) ? t(`develop.wb.${wb.preset}`) : wb.preset;
  }
  const kelvin = new Intl.NumberFormat(language, { maximumFractionDigits: 0 }).format(
    wb.temperature,
  );
  return t("develop.history.custom", {
    temperature: t("develop.kelvin", { value: kelvin }),
    tint: signed(wb.tint, 0, language),
  });
}

/**
 * Libellé lisible d'une étape : le réglage modifié et sa nouvelle valeur.
 * Les étiquettes enregistrées (« light.exposure », « color.whiteBalance.auto »)
 * sont des clés stables, traduites ici.
 */
export function describeStep(
  step: HistoryStep,
  t: TFunction,
  language: string,
  presets: WhitePreset[] = [],
): string {
  const [section, key, auto] = step.label.split(".");
  const s = step.settings;
  if (step.label === "light.auto") return t("develop.history.autoLight");
  if (section === "light" && key) {
    const v = s.light?.[key as keyof NonNullable<DevelopSettings["light"]>] ?? 0;
    const value =
      key === "exposure"
        ? t("develop.ev", { value: signed(v, 2, language) })
        : signed(v, 0, language);
    const name = t(`develop.${key}`);
    return auto
      ? t("develop.history.autoValue", { name, value })
      : t("develop.history.value", { name, value });
  }
  if (section === "color" && key === "whiteBalance") {
    const value = whiteBalance(s, t, language, presets);
    return auto ? t("develop.history.autoWhite", { value }) : t("develop.history.white", { value });
  }
  if (section === "color" && (key === "vibrance" || key === "saturation")) {
    return t("develop.history.value", {
      name: t(`develop.${key}`),
      value: signed(s.color?.[key] ?? 0, 0, language),
    });
  }
  return step.label;
}
