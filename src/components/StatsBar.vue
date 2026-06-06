<template>
  <div
    v-if="stream.isStreaming"
    class="grid grid-cols-3 gap-2 p-3 bg-black/20 rounded-xl border border-white/5 mb-1"
  >
    <div v-for="stat in stats" :key="stat.label" class="flex flex-col items-center gap-1">
      <span class="text-[10px] text-slate-400 uppercase tracking-wider font-medium">{{
        stat.label
      }}</span>
      <span
        class="text-sm font-semibold font-mono text-accent whitespace-nowrap"
        :style="stat.style"
      >
        <span v-if="stat.dot" class="animate-pulse-slow mr-1" :style="{ color: stat.dotColor }"
          >●</span
        >
        {{ stat.value }}
      </span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useStreamStore } from "../stores/stream.ts";

const stream = useStreamStore();

const stats = computed(() => [
  { label: "Uptime", value: stream.formattedUptime, style: undefined },
  { label: "Transferred", value: stream.formattedBytes, style: undefined },
  { label: "Bitrate", value: stream.bitrateKbps.toFixed(1) + " kbps", style: undefined },
  {
    label: "Quality",
    value: `${stream.qualityLabel.text} (${stream.qualityScore})`,
    dot: true,
    dotColor: stream.qualityLabel.color,
    style: undefined,
  },
  { label: "RTT", value: stream.rttMs === null ? "n/a" : stream.rttMs.toFixed(1) + " ms", style: undefined },
  { label: "Underruns", value: String(stream.underruns), style: undefined },
]);
</script>
