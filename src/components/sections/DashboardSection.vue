<template>
  <div class="flex flex-col gap-4">
    <Card>
      <div class="flex items-center justify-between">
        <div class="flex flex-col gap-1">
          <span class="text-xs uppercase tracking-wide text-[var(--color-text-faint)]">
            {{ t("dashboard.statusLabel") }}
          </span>
          <span class="text-2xl font-semibold">{{ stream.statusText }}</span>
        </div>
        <Button :variant="stream.isStreaming ? 'danger' : 'primary'" @click="stream.toggleStream()">
          {{ stream.isStreaming ? t("action.stop") : t("action.start") }}
        </Button>
      </div>
    </Card>

    <div v-if="stream.isStreaming" class="grid grid-cols-3 gap-3">
      <Stat :label="t('stat.uptime')" :value="stream.formattedUptime" />
      <Stat :label="t('stat.transferred')" :value="stream.formattedBytes" />
      <Stat :label="t('stat.bitrate')" :value="stream.bitrateKbps.toFixed(1) + ' kbps'" />
      <Stat
        :label="t('stat.rtt')"
        :value="stream.rttMs === null ? 'n/a' : stream.rttMs.toFixed(1) + ' ms'"
      />
      <Stat :label="t('stat.underruns')" :value="String(stream.underruns)" />
      <Stat :label="t('stat.quality')">
        <span :style="{ color: stream.qualityLabel.color }">
          {{ stream.qualityLabel.text }} ({{ stream.qualityScore }})
        </span>
      </Stat>
    </div>

    <Card :title="t('dashboard.recentLogs')">
      <div class="font-mono text-xs flex flex-col gap-0.5 max-h-40 overflow-auto">
        <div
          v-for="(l, i) in stream.logs.slice(-5)"
          :key="i"
          class="text-[var(--color-text-muted)]"
        >
          <span class="text-[var(--color-text-faint)]">[{{ l.timestamp }}]</span>
          {{ l.message }}
        </div>
        <div v-if="stream.logs.length === 0" class="text-[var(--color-text-faint)]">
          {{ t("logs.empty") }}
        </div>
      </div>
    </Card>
  </div>
</template>
<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { useStreamStore } from "../../stores/stream";
import Card from "../ui/Card.vue";
import Button from "../ui/Button.vue";
import Stat from "../ui/Stat.vue";
const { t } = useI18n();
const stream = useStreamStore();
</script>
