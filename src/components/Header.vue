<template>
  <header
    class="flex items-center justify-between px-4 py-3 border-b border-[var(--color-border)] bg-[var(--color-bg)]"
  >
    <div class="flex items-center gap-3">
      <h2 class="text-sm font-semibold text-[var(--color-text)]">{{ sectionTitle }}</h2>
      <StatusDot :tone="statusTone" :label="stream.statusText" />
    </div>

    <div class="flex items-center gap-2">
      <Button
        :variant="stream.isStreaming ? 'danger' : 'primary'"
        size="sm"
        @click="stream.toggleStream()"
      >
        <IconPlay v-if="!stream.isStreaming" class="w-3.5 h-3.5" />
        <IconStop v-else class="w-3.5 h-3.5" />
        {{ stream.isStreaming ? t("action.stop") : t("action.start") }}
      </Button>

      <Select
        :model-value="ui.locale"
        class="w-[100px]"
        @update:model-value="(v) => ui.setLocale(v as 'en' | 'es')"
      >
        <SelectOption value="en" label="English" />
        <SelectOption value="es" label="Español" />
      </Select>
    </div>
  </header>
</template>
<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useStreamStore } from "../stores/stream";
import { useUiStore } from "../stores/ui";
import Button from "./ui/Button.vue";
import StatusDot from "./ui/StatusDot.vue";
import Select from "./ui/Select.vue";
import SelectOption from "./ui/SelectOption.vue";
import { IconPlay, IconStop } from "./icons";
const { t } = useI18n();
const stream = useStreamStore();
const ui = useUiStore();
const sectionTitle = computed(() => t(`nav.${ui.section}`));
const statusTone = computed(() => {
  if (stream.isStreaming) return "success";
  return "neutral";
});
</script>
