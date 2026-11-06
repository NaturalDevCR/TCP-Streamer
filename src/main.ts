import { createApp, watchEffect } from "vue";
import { createPinia } from "pinia";
import { i18n } from "./i18n";
import App from "./App.vue";
import "./style.css";

const app = createApp(App);
app.use(createPinia());
app.use(i18n);
watchEffect(() => {
  document.documentElement.lang = i18n.global.locale.value;
});
app.mount("#app");
