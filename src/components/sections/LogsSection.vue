<template>
  <div class="flex flex-col gap-4 h-full">
    <Card :title="t('logs.title')">
      <template #header>
        <div class="flex gap-2 items-center">
          <Select v-model="stream.logFilter" class="w-[140px]">
            <SelectOption value="all" :label="t('logs.filterAll')" />
            <SelectOption value="info" :label="t('logs.filterInfo')" />
            <SelectOption value="success" :label="t('logs.filterSuccess')" />
            <SelectOption value="warning" :label="t('logs.filterWarning')" />
            <SelectOption value="error" :label="t('logs.filterError')" />
            <SelectOption value="debug" :label="t('logs.filterDebug')" />
            <SelectOption value="trace" :label="t('logs.filterTrace')" />
          </Select>

          <Input
            v-model="searchQuery"
            :placeholder="t('logs.searchPlaceholder')"
            class="w-[180px]"
          />

          <Button size="sm" variant="ghost" @click="copyLogs">
            {{ t("action.copy") }}
          </Button>
          <Button size="sm" variant="ghost" @click="stream.clearLogs()">
            {{ t("action.clear") }}
          </Button>
        </div>
      </template>

      <div ref="logContainer" class="h-[400px] overflow-y-auto space-y-0.5" @scroll="onScroll">
        <div
          v-for="(log, i) in searchedLogs"
          :key="i"
          :class="[
            'px-2 py-1 rounded text-xs font-mono flex items-start gap-2',
            logColors[log.level] || 'text-[var(--color-text-muted)]',
          ]"
        >
          <span class="text-[var(--color-text-faint)] shrink-0">[{{ log.timestamp }}]</span>
          <span>{{ log.message }}</span>
        </div>
        <div
          v-if="searchedLogs.length === 0"
          class="flex items-center justify-center h-full text-sm text-[var(--color-text-faint)]"
        >
          {{ t("logs.empty") }}
        </div>
      </div>
    </Card>
  </div>
</template>
<script setup lang="ts">
import { ref, computed, watch, nextTick } from "vue";
import { useI18n } from "vue-i18n";
import { useStreamStore } from "../../stores/stream";
import Card from "../ui/Card.vue";
import Button from "../ui/Button.vue";
import Input from "../ui/Input.vue";
import Select from "../ui/Select.vue";
import SelectOption from "../ui/SelectOption.vue";

const { t } = useI18n();
const stream = useStreamStore();

const searchQuery = ref("");
const logContainer = ref<HTMLElement | null>(null);
const userScrolledUp = ref(false);

const searchedLogs = computed(() => {
  if (!searchQuery.value) return stream.filteredLogs;
  const q = searchQuery.value.toLowerCase();
  return stream.filteredLogs.filter(
    (l) =>
      l.message.toLowerCase().includes(q) ||
      l.level.toLowerCase().includes(q) ||
      l.timestamp.includes(q),
  );
});

const logColors: Record<string, string> = {
  info: "text-[var(--color-text-muted)]",
  success: "text-[var(--color-success)]",
  warning: "text-[var(--color-warning)]",
  error: "text-[var(--color-danger)]",
  debug: "text-[var(--color-text-faint)]",
  trace: "text-[var(--color-text-faint)]",
};

watch(
  () => stream.filteredLogs.length,
  async () => {
    if (userScrolledUp.value) return;
    await nextTick();
    if (logContainer.value) {
      logContainer.value.scrollTop = logContainer.value.scrollHeight;
    }
  },
);

function onScroll() {
  if (!logContainer.value) return;
  const el = logContainer.value;
  userScrolledUp.value = el.scrollTop + el.clientHeight < el.scrollHeight - 10;
}

function copyLogs() {
  const text = searchedLogs.value.map((l) => `[${l.timestamp}] ${l.message}`).join("\n");
  navigator.clipboard
    .writeText(text)
    .then(() => stream.addToast(t("toast.copied"), "success"))
    .catch(() => stream.addToast(t("toast.copyFailed"), "error"));
}
</script>
