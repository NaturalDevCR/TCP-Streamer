<template>
  <header
    class="flex flex-wrap gap-2 items-center justify-between px-4 py-2 border-b border-[var(--color-border)] bg-[var(--color-bg)]"
  >
    <div class="flex items-center gap-1">
      <div
        class="w-6 h-6 rounded bg-[var(--color-accent)] flex items-center justify-center text-[10px] font-bold text-white mr-2 shrink-0"
      >
        TS
      </div>

      <nav class="flex items-center gap-0.5">
        <button
          v-for="item in navItems"
          :key="item.id"
          :aria-current="ui.section === item.id ? 'page' : undefined"
          :class="[
            'focus-ring flex items-center gap-1.5 px-3 py-1.5 rounded-md text-[13px] font-medium transition-colors',
            ui.section === item.id
              ? 'bg-[var(--color-surface-3)] text-[var(--color-text)]'
              : 'text-[var(--color-text-muted)] hover:bg-[var(--color-surface-2)] hover:text-[var(--color-text)]',
          ]"
          @click="ui.setSection(item.id)"
        >
          <component :is="item.icon" class="w-[15px] h-[15px] shrink-0" />
          {{ item.label }}
        </button>
      </nav>
    </div>

    <div class="flex flex-wrap min-w-0 max-w-full items-center gap-2">
      <span class="max-w-[180px] truncate" :title="stream.statusText" role="status"
        ><StatusDot :tone="stream.statusTone" :label="stream.statusText"
      /></span>

      <Button
        :variant="stream.pending || stream.isStreaming ? 'danger' : 'primary'"
        size="sm"
        :disabled="stream.stopping || settings.preflightLoading"
        :title="t('operation.shortcuts')"
        @click="stream.toggleStream()"
      >
        <IconPlay v-if="!stream.pending && !stream.isStreaming" class="w-3.5 h-3.5" />
        <IconStop v-else class="w-3.5 h-3.5" />
        {{ stream.pending || stream.isStreaming ? t("action.stop") : t("action.start") }}
      </Button>

      <Button
        v-if="settings.isDirty"
        size="sm"
        variant="secondary"
        :title="t('operation.dirty')"
        @click="settings.saveSettings()"
        >{{ t("operation.save") }}</Button
      >
      <div class="w-[100px] shrink-0">
        <Select
          :aria-label="t('settings.language')"
          :model-value="ui.locale"
          @update:model-value="(v) => ui.setLocale(v as 'en' | 'es')"
        >
          <SelectOption value="en" label="English" />
          <SelectOption value="es" label="Español" />
        </Select>
      </div>
    </div>
  </header>
</template>
<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useStreamStore } from "../stores/stream";
import { useSettingsStore } from "../stores/settings";
import { useUiStore } from "../stores/ui";
import Button from "./ui/Button.vue";
import StatusDot from "./ui/StatusDot.vue";
import Select from "./ui/Select.vue";
import SelectOption from "./ui/SelectOption.vue";
import {
  IconPlay,
  IconStop,
  IconDashboard,
  IconConnection,
  IconAudio,
  IconLogs,
  IconSettings,
} from "./icons";
import type { Section } from "../stores/ui";
const { t } = useI18n();
const stream = useStreamStore();
const ui = useUiStore();
const settings = useSettingsStore();
const navItems = computed<{ id: Section; label: string; icon: typeof IconDashboard }[]>(() => [
  { id: "dashboard", label: t("nav.dashboard"), icon: IconDashboard },
  { id: "connection", label: t("nav.connection"), icon: IconConnection },
  { id: "audio", label: t("nav.audio"), icon: IconAudio },
  { id: "logs", label: t("nav.logs"), icon: IconLogs },
  { id: "settings", label: t("nav.settings"), icon: IconSettings },
]);
</script>
