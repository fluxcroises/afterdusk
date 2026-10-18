import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import en from "./en.json";
import fr from "./fr.json";

export const resources = {
  fr: { translation: fr },
  en: { translation: en },
} as const;

export type Language = keyof typeof resources;

export function systemLanguage(): Language {
  return navigator.language.toLowerCase().startsWith("fr") ? "fr" : "en";
}

void i18n.use(initReactI18next).init({
  resources,
  lng: systemLanguage(),
  fallbackLng: "en",
  interpolation: { escapeValue: false },
});

export default i18n;
