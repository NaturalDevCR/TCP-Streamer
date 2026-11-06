<template>
  <div class="dashboard-layout">
    <section
      class="session-hero"
      :data-tone="stream.statusTone"
      :class="{ 'session-hero-active': stream.isStreaming }"
    >
      <div class="hero-topline">
        <span class="eyebrow"
          ><span
            class="live-dot"
            :class="{ 'is-live': stream.sessionState === 'running' && !stream.stopping }"
          />{{ t("workspace.session") }}</span
        ><Badge>{{ transportLabel }}</Badge>
      </div>
      <div class="hero-heading">
        <div>
          <h2>{{ stream.statusText }}</h2>
          <p>
            {{
              t(
                stream.sessionState === "running" && !stream.stopping
                  ? "workspace.sessionActive"
                  : stream.pending || stream.isStreaming
                    ? "workspace.sessionPending"
                    : "workspace.readyDescription",
              )
            }}
          </p>
        </div>
        <span class="hero-wave" aria-hidden="true"><IconWave /></span>
      </div>
      <div class="signal-path" :aria-label="t('workspace.signalPath')">
        <div class="signal-node">
          <span class="signal-node-icon"
            ><component :is="isSink ? IconConnection : IconAudio" /></span
          ><span class="signal-node-copy"
            ><small>{{ t(isSink ? "connection.sourceAddress" : "audio.inputDevice") }}</small
            ><strong>{{ firstEndpoint || t("workspace.notConfigured") }}</strong></span
          >
        </div>
        <div class="signal-link" aria-hidden="true"><span /><IconArrowRight /></div>
        <div class="signal-node signal-app">
          <span class="signal-node-icon"><IconWave /></span
          ><span class="signal-node-copy"
            ><small>TCP Streamer</small
            ><strong>{{
              t(isSink ? "connection.roleSink" : "connection.roleSource")
            }}</strong></span
          >
        </div>
        <div class="signal-link" aria-hidden="true"><span /><IconArrowRight /></div>
        <div class="signal-node">
          <span class="signal-node-icon"
            ><component :is="isSink ? IconAudio : IconConnection" /></span
          ><span class="signal-node-copy"
            ><small>{{ t(isSink ? "connection.outputDevice" : "workspace.destination") }}</small
            ><strong>{{ lastEndpoint || t("workspace.notConfigured") }}</strong></span
          >
        </div>
      </div>
      <div class="hero-bottom">
        <span
          ><IconLock v-if="encrypted" /><IconConnection v-else />{{
            t(encrypted ? "workspace.encryptedTransport" : "workspace.unencryptedTransport")
          }}</span
        ><Button size="sm" variant="ghost" @click="ui.setSection('connection')"
          >{{ t(stream.isStreaming ? "connection.info" : "workspace.configureConnection")
          }}<IconArrowRight class="w-3.5 h-3.5"
        /></Button>
      </div>
    </section>

    <div class="overview-stats">
      <Stat :label="t('stat.uptime')" :value="hasStats ? stream.formattedUptime : '—'"
        ><template #icon><IconClock /></template
      ></Stat>
      <Stat :label="t('stat.transferred')" :value="hasStats ? stream.formattedBytes : '—'"
        ><template #icon><IconTransfer /></template
      ></Stat>
      <Stat
        :label="t('stat.bitrate')"
        :value="hasStats ? stream.bitrateKbps.toFixed(1) + ' kbps' : '—'"
        ><template #icon><IconActivity /></template
      ></Stat>
      <Stat
        :label="t('stat.rtt')"
        :value="
          stream.isStreaming && stream.qualityAvailable && stream.rttMs !== null
            ? stream.rttMs.toFixed(1) + ' ms'
            : '—'
        "
        ><template #icon><IconConnection /></template
      ></Stat>
    </div>

    <div class="dashboard-columns">
      <Card :title="t('workspace.streamSetup')" class="setup-card">
        <template #header
          ><button class="text-link focus-ring" @click="ui.setSection('audio')">
            {{ t("workspace.configure") }}<IconArrowRight /></button
        ></template>
        <dl class="summary-list">
          <div>
            <dt>{{ t("connection.role") }}</dt>
            <dd>{{ t(isSink ? "connection.roleSink" : "connection.roleSource") }}</dd>
          </div>
          <div>
            <dt>{{ t("connection.transport") }}</dt>
            <dd>{{ transportLabel }}</dd>
          </div>
          <div>
            <dt>{{ t("workspace.currentProfile") }}</dt>
            <dd>{{ settings.currentProfile }}</dd>
          </div>
          <div>
            <dt>{{ t("audio.latencyProfile") }}</dt>
            <dd>{{ profileLabel(String(currentConfig?.profile ?? settings.latencyProfile)) }}</dd>
          </div>
          <div>
            <dt>{{ t("operation.wireFormat") }}</dt>
            <dd>
              {{
                currentConfig
                  ? `${currentConfig.sample_rate / 1000} kHz · ${currentConfig.channels} ch`
                  : isSink
                    ? t("workspace.negotiatedAtStart")
                    : `${settings.sampleRate / 1000} kHz · s16le · 2 ch`
              }}
            </dd>
          </div>
        </dl>
      </Card>
      <Card :title="t('workspace.streamHealth')" class="health-card">
        <template #header
          ><Badge
            :tone="
              stream.isStreaming && stream.qualityAvailable
                ? stream.qualityScore >= 70
                  ? 'success'
                  : 'warning'
                : 'neutral'
            "
            >{{
              stream.isStreaming && stream.qualityAvailable
                ? stream.qualityLabel.text
                : t("workspace.standby")
            }}</Badge
          ></template
        >
        <template v-if="stream.isStreaming && stream.qualityAvailable"
          ><div class="health-score">
            <strong>{{ stream.qualityScore }}<small>/100</small></strong
            ><span>{{ t("stat.quality") }}</span>
          </div>
          <div
            class="health-track"
            role="meter"
            :aria-label="t('stat.quality')"
            :aria-valuenow="stream.qualityScore"
            aria-valuemin="0"
            aria-valuemax="100"
          >
            <span
              :style="{
                width: `${Math.max(0, Math.min(100, stream.qualityScore))}%`,
                background: stream.qualityLabel.color,
              }"
            />
          </div>
          <div class="health-occupancy">
            <span>{{ t("operation.ringOccupancy") }}</span
            ><b>{{ (stream.bufferHealth * 100).toFixed(1) }}%</b>
          </div></template
        >
        <div v-else class="health-empty">
          <span class="health-empty-icon"><IconActivity /></span>
          <p>{{ t("workspace.waitingForMetrics") }}</p>
          <small>{{ t("workspace.metricsDescription") }}</small>
        </div>
      </Card>
    </div>

    <div v-if="stream.isStreaming" class="diagnostic-stats">
      <Stat
        :label="
          t(
            stream.qualityUnits === 'frames/frames'
              ? 'operation.missingAudioFrames'
              : 'operation.underrunEvents',
          )
        "
        :value="stream.qualityAvailable ? String(stream.underruns) : '—'"
      />
      <Stat
        :label="t('operation.droppedAudioFrames')"
        :value="stream.qualityAvailable ? String(stream.dropped) : '—'"
      />
      <Stat
        :label="t('operation.ringOccupancy')"
        :value="stream.qualityAvailable ? `${(stream.bufferHealth * 100).toFixed(1)}%` : '—'"
      />
    </div>
    <div v-if="hasStats && stream.sinkMetrics.received" class="diagnostic-stats">
      <Stat
        :label="t('operation.authenticationFailures')"
        :value="String(stream.sinkMetrics.authFailures)"
      />
      <Stat :label="t('operation.lostFrames')" :value="String(stream.sinkMetrics.lostFrames)" />
      <Stat :label="t('operation.drift')" :value="`${stream.sinkMetrics.trimPpm} ppm`" />
    </div>
    <Card v-if="stream.isStreaming" class="effective-config" :title="t('operation.effective')">
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

    <Card :title="t('dashboard.recentLogs')" class="recent-activity">
      <template #header
        ><button class="text-link focus-ring" @click="ui.setSection('logs')">
          {{ t("workspace.viewAll") }}<IconArrowRight /></button
      ></template>
      <div v-if="stream.logs.length" class="recent-log-list">
        <div v-for="(log, i) in stream.logs.slice(-4).reverse()" :key="i" class="recent-log">
          <span class="log-level-dot" :data-level="log.level" /><time>{{ log.timestamp }}</time
          ><span>{{ log.message }}</span>
        </div>
      </div>
      <div v-else class="activity-empty">
        <IconLogs /><span>{{ t("workspace.noActivity") }}</span
        ><small>{{ t("workspace.activityDescription") }}</small>
      </div>
    </Card>
    <p class="tray-note"><IconInfo />{{ t("operation.close") }}</p>
  </div>
</template>
<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settings";
import { useStreamStore } from "../../stores/stream";
import { useUiStore } from "../../stores/ui";
import Card from "../ui/Card.vue";
import Button from "../ui/Button.vue";
import Badge from "../ui/Badge.vue";
import Stat from "../ui/Stat.vue";
import {
  IconWave,
  IconAudio,
  IconConnection,
  IconArrowRight,
  IconLock,
  IconClock,
  IconTransfer,
  IconActivity,
  IconLogs,
  IconInfo,
} from "../icons";
const { t } = useI18n();
const stream = useStreamStore();
const settings = useSettingsStore();
const ui = useUiStore();
const currentConfig = computed(() => (stream.isStreaming ? stream.effectiveConfig : null));
const active = computed(() => (stream.isStreaming ? stream.activeSettings : null));
const isSink = computed(() => (active.value?.role ?? settings.role) === "sink");
const transportLabel = computed(() =>
  isSink.value || (active.value?.transport ?? settings.transport) === "udp"
    ? t("connection.transportUdp")
    : "TCP",
);
const firstEndpoint = computed(() =>
  String(
    isSink.value
      ? (active.value?.source_addr ?? settings.sourceAddr)
      : (active.value?.device ?? settings.deviceName),
  ),
);
const lastEndpoint = computed(() => {
  if (isSink.value) return String(active.value?.output_device ?? settings.outputDevice);
  const transport = active.value?.transport ?? settings.transport;
  const mode = active.value?.mode ?? settings.mode;
  const port = active.value?.port ?? settings.port;
  if (transport === "udp" || mode === "server")
    return `${settings.localIp.includes(":") ? `[${settings.localIp}]` : settings.localIp}:${port}`;
  const host = String(active.value?.ip ?? settings.ip);
  return host ? `${host.includes(":") && !host.startsWith("[") ? `[${host}]` : host}:${port}` : "";
});
const encrypted = computed(
  () =>
    transportLabel.value !== "TCP" &&
    (active.value ? Boolean(active.value.encrypted) : Boolean(settings.psk)),
);
const hasStats = computed(() => stream.isStreaming && stream.statsAvailable);
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
