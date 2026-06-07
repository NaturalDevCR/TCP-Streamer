import { defineStore } from "pinia";
import { ref } from "vue";
import { i18n } from "../i18n";

export type Section = "dashboard" | "connection" | "audio" | "logs" | "settings";

export const useUiStore = defineStore("ui", () => {
  const section = ref<Section>("dashboard");
  const locale = ref<"en" | "es">(i18n.global.locale.value as "en" | "es");
  const theme = ref<"dark">("dark");

  function setSection(s: Section) {
    section.value = s;
  }
  function setLocale(l: "en" | "es") {
    locale.value = l;
    i18n.global.locale.value = l;
  }
  return { section, locale, theme, setSection, setLocale };
});
