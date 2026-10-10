import { createI18n } from "vue-i18n";
import en from "./en.json";
import es from "./es.json";

function defaultLocale(): "en" | "es" {
  try {
    const saved = localStorage.getItem("tcp-streamer-locale");
    if (saved === "en" || saved === "es") return saved;
  } catch {
    /* Storage unavailable */
  }
  const nav = typeof navigator !== "undefined" ? navigator.language : "en";
  return nav.toLowerCase().startsWith("es") ? "es" : "en";
}

export const i18n = createI18n({
  legacy: false,
  locale: defaultLocale(),
  fallbackLocale: "en",
  messages: { en, es },
});
