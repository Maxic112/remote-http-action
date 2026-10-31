import i18n from "i18next";
import { initReactI18next } from "react-i18next";

import en from "./en";
import ru from "./ru";

export type Language = "system" | "ru" | "en";

export function resolveLanguage(setting: Language, browser = navigator.language): "ru" | "en" {
  if (setting === "ru" || setting === "en") return setting;
  return browser.toLowerCase().startsWith("ru") ? "ru" : "en";
}

void i18n.use(initReactI18next).init({
  resources: { ru: { translation: ru }, en: { translation: en } },
  lng: resolveLanguage("system"),
  fallbackLng: "en",
  interpolation: { escapeValue: false },
});

export default i18n;
