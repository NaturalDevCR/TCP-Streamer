<template>
  <aside
    class="flex flex-col w-[200px] shrink-0 bg-[var(--color-surface-1)] border-r border-[var(--color-border)]"
  >
    <div class="flex items-center gap-2 px-4 py-3 border-b border-[var(--color-border)]">
      <div
        class="w-6 h-6 rounded bg-[var(--color-accent)] flex items-center justify-center text-[10px] font-bold text-white"
      >
        TS
      </div>
      <span class="text-sm font-semibold text-[var(--color-text)]">{{ t("app.name") }}</span>
    </div>

    <nav class="flex-1 flex flex-col gap-0.5 p-2">
      <button
        v-for="item in navItems"
        :key="item.id"
        :class="[
          'focus-ring flex items-center gap-2.5 px-3 py-2 rounded-md text-sm font-medium transition-colors text-left w-full',
          ui.section === item.id
            ? 'bg-[var(--color-surface-3)] text-[var(--color-text)]'
            : 'text-[var(--color-text-muted)] hover:bg-[var(--color-surface-2)] hover:text-[var(--color-text)]',
        ]"
        @click="ui.setSection(item.id)"
      >
        <component :is="item.icon" class="w-[18px] h-[18px] shrink-0" />
        {{ item.label }}
      </button>
    </nav>

    <div class="px-4 py-3 border-t border-[var(--color-border)]">
      <span class="text-xs text-[var(--color-text-faint)]"> v{{ version }} </span>
    </div>
  </aside>
</template>
<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { useUiStore } from "../stores/ui";
import { IconDashboard, IconConnection, IconAudio, IconLogs, IconSettings } from "./icons";
import type { Section } from "../stores/ui";
const { t } = useI18n();
const ui = useUiStore();
const version = import.meta.env.PACKAGE_VERSION || "0.0.0";
const navItems: { id: Section; label: string; icon: typeof IconDashboard }[] = [
  { id: "dashboard", label: t("nav.dashboard"), icon: IconDashboard },
  { id: "connection", label: t("nav.connection"), icon: IconConnection },
  { id: "audio", label: t("nav.audio"), icon: IconAudio },
  { id: "logs", label: t("nav.logs"), icon: IconLogs },
  { id: "settings", label: t("nav.settings"), icon: IconSettings },
];
</script>
