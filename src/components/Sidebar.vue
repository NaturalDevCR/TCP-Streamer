<template>
  <aside class="sidebar">
    <a class="brand focus-ring" href="#main-content" @click.prevent="ui.setSection('dashboard')">
      <span class="brand-mark"><IconWave /></span>
      <span class="brand-name"
        >TCP Streamer<span>{{ t("workspace.audioWorkspace") }}</span></span
      >
    </a>

    <div class="sidebar-group-label">{{ t("workspace.workspace") }}</div>
    <nav class="sidebar-nav" :aria-label="t('workspace.navigation')">
      <button
        v-for="item in navItems"
        :key="item.id"
        :aria-current="ui.section === item.id ? 'page' : undefined"
        :aria-label="item.label"
        :title="item.label"
        class="nav-item focus-ring"
        @click="ui.setSection(item.id)"
      >
        <component :is="item.icon" />
        <span>{{ item.label }}</span>
        <span v-if="item.id === 'logs' && stream.logs.length" class="nav-count">{{
          stream.logs.length
        }}</span>
        <span v-else-if="ui.section === item.id" class="nav-active-dot" />
      </button>
    </nav>

    <div class="sidebar-session">
      <div class="sidebar-group-label">{{ t("workspace.session") }}</div>
      <div class="session-indicator">
        <StatusDot :tone="stream.statusTone" /><span>{{ stream.statusText }}</span>
      </div>
      <p>{{ t(stream.isStreaming ? "workspace.sessionActive" : "workspace.sessionIdle") }}</p>
      <div class="session-transport">
        <span>{{ settings.role === "sink" ? "UDP" : settings.transport.toUpperCase() }}</span
        ><span>PCM / STEREO</span>
      </div>
    </div>

    <button
      class="profile-shortcut focus-ring"
      :title="t('settings.profiles')"
      @click="ui.setSection('settings')"
    >
      <span class="profile-avatar">{{ settings.currentProfile.slice(0, 1).toUpperCase() }}</span>
      <span class="profile-copy"
        ><span>{{ settings.currentProfile }}</span
        ><small>{{ t("workspace.currentProfile") }}</small></span
      >
      <IconChevronRight class="profile-chevron" />
    </button>
    <div class="sidebar-version">
      <span>TCP STREAMER</span><span>v{{ version }}</span>
    </div>
  </aside>
</template>
<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useUiStore, type Section } from "../stores/ui";
import { useSettingsStore } from "../stores/settings";
import { useStreamStore } from "../stores/stream";
import StatusDot from "./ui/StatusDot.vue";
import {
  IconDashboard,
  IconConnection,
  IconAudio,
  IconLogs,
  IconSettings,
  IconWave,
  IconChevronRight,
} from "./icons";
const { t } = useI18n();
const ui = useUiStore();
const settings = useSettingsStore();
const stream = useStreamStore();
const version = import.meta.env.PACKAGE_VERSION;
const navItems = computed(() => [
  { id: "dashboard" as Section, label: t("nav.dashboard"), icon: IconDashboard },
  { id: "connection" as Section, label: t("nav.connection"), icon: IconConnection },
  { id: "audio" as Section, label: t("nav.audio"), icon: IconAudio },
  { id: "logs" as Section, label: t("nav.logs"), icon: IconLogs },
  { id: "settings" as Section, label: t("nav.settings"), icon: IconSettings },
]);
</script>
