import type { TFunction } from "i18next";
import {
  type DevelopSettings,
  DETAIL_DEFAULTS,
  type DetailGroup,
  EFFECT_DEFAULTS,
  type EffectGroup,
  type HistoryStep,
  type HslKind,
  type Hue,
  type WhitePreset,
} from "../store/develop";

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
  if (step.label.startsWith("snapshot:")) {
    return t("develop.history.snapshot", { name: step.label.slice("snapshot:".length) });
  }
  if (step.label === "version.create") return t("develop.history.versionCreated");
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
  if (section === "geometry" && key) {
    const angle = new Intl.NumberFormat(language, {
      signDisplay: "exceptZero",
      minimumFractionDigits: 1,
      maximumFractionDigits: 1,
    }).format(s.geometry?.angle ?? 0);
    const aspect = s.geometry?.aspect ?? "free";
    return t(`develop.history.geometry.${key}`, {
      value:
        key === "aspect"
          ? aspect.includes(":")
            ? aspect
            : t(`develop.crop.aspects.${aspect}`)
          : angle,
    });
  }
  if (section === "curve" && key) {
    return t("develop.history.curve", { channel: t(`develop.curve.${key}`) });
  }
  if (section === "hsl" && key && auto) {
    const hue = auto as Hue;
    return t("develop.history.hsl", {
      kind: t(`develop.hsl.${key}`),
      hue: t(`develop.hues.${hue}`),
      value: signed(s.hsl?.[key as HslKind]?.[hue] ?? 0, 0, language),
    });
  }
  if (section === "detail" && key && auto) {
    const group = key as DetailGroup;
    const fallback = (DETAIL_DEFAULTS[group] as Record<string, number>)[auto] ?? 0;
    const v = (s.detail?.[group] as Record<string, number> | undefined)?.[auto] ?? fallback;
    return t("develop.history.effect", {
      group: t(`develop.detail.${group}`),
      name: t(`develop.detail.${group}_${auto}`).toLowerCase(),
      value: new Intl.NumberFormat(language).format(v),
    });
  }
  if (section === "effects" && key && auto) {
    const group = key as EffectGroup;
    const fallback = (EFFECT_DEFAULTS[group] as Record<string, number>)[auto] ?? 0;
    const v = (s.effects?.[group] as Record<string, number> | undefined)?.[auto] ?? fallback;
    return t("develop.history.effect", {
      group: t(`develop.effects.${group}`),
      name: t(`develop.effects.${group}_${auto}`).toLowerCase(),
      value: group === "vignette" && auto === "amount" ? signed(v, 0, language) : String(v),
    });
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
