import type { TFunction } from "i18next";

/** Nom d'un ratio de recadrage : « Libre », « Original », « Carré », sinon « 4:5 ». */
export function aspectLabel(t: TFunction, aspect: string): string {
  if (aspect === "free" || aspect === "original" || aspect === "1:1") {
    return t(`develop.crop.aspects.${aspect === "1:1" ? "square" : aspect}`);
  }
  return aspect;
}
