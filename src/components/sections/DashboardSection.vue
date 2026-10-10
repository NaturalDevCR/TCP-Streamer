<template>
  <div class="flex flex-col gap-4">
    <Card>
      <div class="flex items-center justify-between">
        <div class="flex min-w-0 flex-col gap-1">
          <span class="text-xs uppercase tracking-wide text-[var(--color-text-faint)]">
            {{ t("dashboard.statusLabel") }}
          </span>
          <span
            role="status"
            class="text-2xl font-semibold break-words"
            :class="{
              'text-[var(--color-danger)]': stream.statusTone === 'danger',
              'text-[var(--color-warning)]': stream.statusTone === 'warning',
              'text-[var(--color-success)]': stream.statusTone === 'success',
            }"
            >{{ stream.statusText }}</span
          >
        </div>
        <Button
          :disabled="stream.stopping || settings.preflightLoading"
          :variant="stream.pending || stream.isStreaming ? 'danger' : 'primary'"
          @click="stream.toggleStream()"
        >
          {{ stream.pending || stream.isStreaming ? t("action.stop") : t("action.start") }}
        </Button>
      </div>
    </Card>

    <div v-if="stream.isStreaming" class="grid grid-cols-3 gap-3">
      <Stat
        :label="t('stat.uptime')"
        :value="stream.statsAvailable ? stream.formattedUptime : t('operation.noData')"
      />
      <Stat
        :label="t('stat.transferred')"
        :value="stream.statsAvailable ? stream.formattedBytes : t('operation.noData')"
      />
      <Stat
        :label="t('stat.bitrate')"
        :value="
          stream.statsAvailable ? stream.bitrateKbps.toFixed(1) + ' kbps' : t('operation.noData')
        "
      />
      <Stat
        :label="t('stat.rtt')"
        :value="stream.rttMs === null ? t('operation.noData') : stream.rttMs.toFixed(1) + ' ms'"
      />
      <Stat
        :label="
          t(
            stream.qualityUnits === 'frames/frames'
              ? 'operation.missingAudioFrames'
              : 'operation.underrunEvents',
          )
        "
        :value="stream.qualityAvailable ? String(stream.underruns) : t('operation.noData')"
      />
      <Stat
        :label="t('operation.droppedAudioFrames')"
        :value="stream.qualityAvailable ? String(stream.dropped) : t('operation.noData')"
      />
      <Stat
        :label="t('operation.ringOccupancy')"
        :value="
          stream.qualityAvailable
            ? `${(stream.bufferHealth * 100).toFixed(1)}%`
            : t('operation.noData')
        "
      />
      <Stat :label="t('stat.quality')">
        <span v-if="stream.qualityAvailable" :style="{ color: stream.qualityLabel.color }">
          {{ stream.qualityLabel.text }} ({{ stream.qualityScore }}) </span
        ><span v-else>{{ t("operation.noData") }}</span>
      </Stat>
    </div>

    <div v-if="stream.statsAvailable && stream.sinkMetrics.received" class="grid grid-cols-3 gap-3">
      <Stat
        :label="t('operation.authenticationFailures')"
        :value="String(stream.sinkMetrics.authFailures)"
      />
      <Stat :label="t('operation.lostFrames')" :value="String(stream.sinkMetrics.lostFrames)" />
      <Stat :label="t('operation.drift')" :value="`${stream.sinkMetrics.trimPpm} ppm`" />
    </div>
    <p v-if="stream.isStreaming" class="text-xs text-[var(--color-text-muted)]">
      {{ t("operation.locked") }}
    </p>
    <Card v-if="stream.isStreaming" :title="t('operation.effective')">
      <div v-if="stream.effectiveConfig" class="text-sm space-y-2">
        <p>
          {{
            t(
              stream.effectiveConfig.role === "sink"
                ? "connection.roleSink"
                : "connection.roleSource",
            )
          }}
          · {{ stream.effectiveConfig.transport.toUpperCase() }} ·
          {{ profileLabel(stream.effectiveConfig.profile) }}
        </p>
        <dl class="grid grid-cols-2 gap-x-4 gap-y-1">
          <dt>{{ t("operation.wireFormat") }}</dt>
          <dd>
            {{ stream.effectiveConfig.sample_rate }} Hz · {{ stream.effectiveConfig.channels }} ch
          </dd>
          <dt>{{ t("operation.deviceFormat") }}</dt>
          <dd>
            {{ stream.effectiveConfig.device_sample_rate }} Hz ·
            {{ stream.effectiveConfig.device_channels }} ch
          </dd>
          <dt>{{ t("operation.ringCapacity") }}</dt>
          <dd>{{ stream.effectiveConfig.ring_ms }} ms</dd>
          <dt>{{ t("operation.targetLatency") }}</dt>
          <dd>{{ stream.effectiveConfig.target_ms }} ms</dd>
          <template v-if="stream.effectiveConfig.role === 'source'">
            <dt>{{ t("audio.chunkSize") }}</dt>
            <dd>{{ stream.effectiveConfig.chunk_frames }} {{ t("operation.frames") }}</dd>
            <dt>{{ t("audio.bufferSize") }}</dt>
            <dd>
              {{ stream.effectiveConfig.capture_buffer_frames ?? t("operation.driverDefault") }}
            </dd>
            <dt>{{ t("audio.adaptiveBuffer") }}</dt>
            <dd>
              {{ t(stream.effectiveConfig.adaptive ? "operation.enabled" : "operation.disabled") }}
            </dd>
          </template>
        </dl>
      </div>
      <p v-else class="text-sm">{{ t("operation.noData") }}</p>
      <p v-if="stream.statsAvailable && stream.effectiveFormat.sample_rate" class="text-xs mt-2">
        {{
          t(
            stream.effectiveConfig?.role === "sink"
              ? "operation.currentOutput"
              : "operation.currentWire",
          )
        }}: {{ stream.effectiveFormat.sample_rate }} Hz · {{ stream.effectiveFormat.channels }} ch ·
        {{ t("operation.occupancy") }}: {{ stream.effectiveFormat.buffer_ms?.toFixed(1) }} ms ·
        {{ t("operation.targetLatency") }}: {{ stream.effectiveFormat.target_ms }} ms
      </p>
      <p v-if="stream.activeSettings" class="text-xs text-[var(--color-text-muted)] mt-2">
        {{ t("operation.requested") }}:
        {{
          stream.activeSettings.role === "sink"
            ? stream.activeSettings.output_device
            : stream.activeSettings.device
        }}
        · {{ profileLabel(String(stream.activeSettings.latency_profile))
        }}<template v-if="stream.activeSettings.role === 'source'">
          · {{ stream.activeSettings.sample_rate }} Hz · {{ stream.activeSettings.buffer_size }}
          {{ t("operation.frames") }}</template
        >
      </p>
    </Card>
    <p v-if="!stream.pending && !stream.isStreaming" class="text-sm text-[var(--color-text-muted)]">
      {{ t("operation.onboarding") }}
    </p>
    <p class="text-xs text-[var(--color-text-faint)]">{{ t("operation.close") }}</p>
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
import { useSettingsStore } from "../../stores/settings";
import { useStreamStore } from "../../stores/stream";
import Card from "../ui/Card.vue";
import Button from "../ui/Button.vue";
import Stat from "../ui/Stat.vue";
const { t } = useI18n();
const stream = useStreamStore();
const settings = useSettingsStore();
function profileLabel(value: string) {
  const keys: Record<string, string> = {
    "ultra-low": "audio.latencyUltraLow",
    balanced: "audio.latencyBalanced",
    robust: "audio.latencyRobust",
    broadcast: "audio.latencyBroadcast",
    custom: "audio.latencyCustom",
  };
  return keys[value] ? t(keys[value]) : value;
}
</script>
