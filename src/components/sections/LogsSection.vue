<template>
  <div class="logs-section">
    <Card :title="t('workspace.eventLog')" class="log-panel">
      <template #header
        ><span class="log-follow-status"
          ><span :class="{ 'is-following': !paused && !userScrolledUp }" />{{
            t(paused || userScrolledUp ? "workspace.followPaused" : "workspace.following")
          }}</span
        ></template
      >
      <div class="log-toolbar">
        <Select v-model="stream.logFilter" :aria-label="t('workspace.logLevel')" class="log-filter">
          <SelectOption value="all" :label="t('logs.filterAll')" /><SelectOption
            value="info"
            :label="t('logs.filterInfo')"
          /><SelectOption value="success" :label="t('logs.filterSuccess')" /><SelectOption
            value="warning"
            :label="t('logs.filterWarning')"
          /><SelectOption value="error" :label="t('logs.filterError')" /><SelectOption
            value="debug"
            :label="t('logs.filterDebug')"
          /><SelectOption value="trace" :label="t('logs.filterTrace')" />
        </Select>
        <div class="log-search">
          <IconSearch aria-hidden="true" /><Input
            v-model="searchQuery"
            :placeholder="t('logs.searchPlaceholder')"
            :aria-label="t('logs.searchPlaceholder')"
          />
        </div>
        <div class="log-actions">
          <Button size="sm" variant="secondary" :aria-pressed="paused" @click="toggleFollowing">{{
            t(paused ? "operation.paused" : "operation.pause")
          }}</Button
          ><Button
            size="sm"
            variant="ghost"
            :title="t('action.copy')"
            :aria-label="t('action.copy')"
            @click="copyLogs"
            ><IconCopy class="w-4 h-4" /></Button
          ><Button size="sm" variant="ghost" @click="exportLogs">{{ t("operation.export") }}</Button
          ><Button
            size="sm"
            variant="ghost"
            :title="t('action.clear')"
            :aria-label="t('action.clear')"
            @click="stream.clearLogs()"
            ><IconTrash class="w-4 h-4"
          /></Button>
        </div>
      </div>
      <div class="log-column-labels" aria-hidden="true">
        <span>{{ t("workspace.time") }}</span
        ><span>{{ t("workspace.level") }}</span
        ><span>{{ t("workspace.message") }}</span>
      </div>
      <div
        ref="logContainer"
        class="log-viewport focus-ring"
        tabindex="0"
        :aria-label="t('logs.title')"
        @scroll="onScroll"
      >
        <div v-for="(log, i) in searchedLogs" :key="i" class="log-row">
          <time>{{ log.timestamp }}</time
          ><span class="log-level" :data-level="log.level">{{ log.level }}</span
          ><span :class="logColors[log.level]">{{ log.message }}</span>
        </div>
        <div v-if="searchedLogs.length === 0" class="log-empty">
          <span><IconLogs /></span>
          <h3>{{ t(searchQuery ? "workspace.noMatchingLogs" : "workspace.noActivity") }}</h3>
          <p>{{ t(searchQuery ? "workspace.adjustFilters" : "workspace.activityDescription") }}</p>
        </div>
      </div>
      <div class="log-footer">
        <span>{{ t("workspace.logCount", { count: searchedLogs.length }) }}</span
        ><span>{{ t("workspace.logRetention") }}</span>
      </div>
    </Card>
  </div>
</template>
<script setup lang="ts">
import { ref, computed, watch, nextTick } from "vue";
import { useI18n } from "vue-i18n";
import { sanitizeSupport } from "../../utils/sanitize";
import { useSettingsStore } from "../../stores/settings";
import { useStreamStore } from "../../stores/stream";
import { IconSearch, IconCopy, IconTrash, IconLogs } from "../icons";
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
const paused = ref(false);

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
  () => stream.filteredLogs.at(-1),
  async () => {
    if (userScrolledUp.value || paused.value) return;
    await nextTick();
    if (logContainer.value) {
      logContainer.value.scrollTop = logContainer.value.scrollHeight;
    }
  },
);

async function toggleFollowing() {
  paused.value = !paused.value;
  if (!paused.value) {
    userScrolledUp.value = false;
    await nextTick();
    if (logContainer.value) logContainer.value.scrollTop = logContainer.value.scrollHeight;
  }
}

function onScroll() {
  if (!logContainer.value) return;
  const el = logContainer.value;
  userScrolledUp.value = el.scrollTop + el.clientHeight < el.scrollHeight - 10;
}

function exportLogs() {
  const settings = useSettingsStore();
  const values = [
    settings.psk,
    settings.ip,
    settings.sourceAddr,
    settings.allowlist,
    ...settings.discoveredSources.map((s) => s.addr),
    ...Object.values(settings.profiles).flatMap((p) => [
      String(p.psk || ""),
      String(p.ip || ""),
      String(p.source_addr || ""),
    ]),
  ];
  const text = searchedLogs.value
    .map((l) => `[${l.timestamp}] [${l.level}] ${l.message}`)
    .join("\n");
  navigator.clipboard
    .writeText(sanitizeSupport(text, values))
    .then(() => stream.addToast(t("toast.copied")))
    .catch(() => stream.addToast(t("toast.copyFailed"), "error"));
}
function copyLogs() {
  const text = searchedLogs.value
    .map((l) => `[${l.timestamp}] [${l.level}] ${l.message}`)
    .join("\n");
  navigator.clipboard
    .writeText(text)
    .then(() => stream.addToast(t("toast.copied"), "success"))
    .catch(() => stream.addToast(t("toast.copyFailed"), "error"));
}
</script>
