<template>
  <ToastNotification />
  <div class="app-shell">
    <a class="skip-link focus-ring" href="#main-content">{{ t("workspace.skipNavigation") }}</a>
    <Sidebar />
    <div class="workspace">
      <Header />
      <main id="main-content" ref="mainContent" class="workspace-main" tabindex="-1">
        <div class="page-content">
          <div class="page-heading">
            <div>
              <p class="eyebrow">{{ t("workspace.audioControl") }}</p>
              <h1>{{ t(`workspace.titles.${ui.section}`) }}</h1>
              <p class="page-description">{{ t(`workspace.descriptions.${ui.section}`) }}</p>
            </div>
            <span class="page-heading-icon" aria-hidden="true"
              ><component :is="sectionIcon"
            /></span>
          </div>
          <div
            v-if="
              settings.configurationLocked && ui.section !== 'dashboard' && ui.section !== 'logs'
            "
            class="notice notice-warning"
          >
            <IconLock />{{ t("operation.locked") }}
          </div>
          <div v-if="!settings.nativeAvailable" class="notice browser-preview-note" role="note">
            <IconInfo />
            <div>
              <strong>{{ t("runtime.preview") }}</strong>
              <p>{{ t("runtime.previewDescription") }}</p>
            </div>
          </div>
          <DashboardSection v-if="ui.section === 'dashboard'" />
          <ConnectionSection v-if="ui.section === 'connection'" />
          <AudioSection v-if="ui.section === 'audio'" />
          <LogsSection v-if="ui.section === 'logs'" />
          <SettingsSection v-if="ui.section === 'settings'" />
        </div>
      </main>
      <footer class="workspace-footer">
        <span
          ><span class="footer-dot" />{{ t("workspace.localAddress") }}
          <b>{{ settings.nativeAvailable ? settings.localIp : "—" }}</b></span
        >
        <span v-if="settings.nativeAvailable" class="footer-shortcuts"
          ><kbd>{{ modifier }} S</kbd> {{ t("action.save") }}<span class="footer-divider" /><kbd
            >{{ modifier }} ↵</kbd
          >
          {{ t("workspace.startStop") }}</span
        >
      </footer>
    </div>
  </div>
</template>
<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useUiStore } from "../stores/ui";
import { useSettingsStore } from "../stores/settings";
import ToastNotification from "./ToastNotification.vue";
import Header from "./Header.vue";
import Sidebar from "./Sidebar.vue";
import DashboardSection from "./sections/DashboardSection.vue";
import ConnectionSection from "./sections/ConnectionSection.vue";
import AudioSection from "./sections/AudioSection.vue";
import LogsSection from "./sections/LogsSection.vue";
import SettingsSection from "./sections/SettingsSection.vue";
import {
  IconDashboard,
  IconConnection,
  IconAudio,
  IconLogs,
  IconSettings,
  IconLock,
  IconInfo,
} from "./icons";
const { t } = useI18n();
const ui = useUiStore();
const settings = useSettingsStore();
const mainContent = ref<HTMLElement | null>(null);
const modifier = /Mac/i.test(navigator.platform) ? "⌘" : "Ctrl";
const sectionIcon = computed(
  () =>
    ({
      dashboard: IconDashboard,
      connection: IconConnection,
      audio: IconAudio,
      logs: IconLogs,
      settings: IconSettings,
    })[ui.section],
);
watch(
  () => ui.section,
  () => {
    if (!mainContent.value) return;
    mainContent.value.scrollTop = 0;
    mainContent.value.focus({ preventScroll: true });
  },
  { flush: "post" },
);
</script>
