<template>
  <div class="animate-fade-in">
    <section class="glass-card">
      <!-- Header -->
      <div class="flex items-center justify-between border-b border-white/10 pb-2 mb-3">
        <h3 class="text-sm font-semibold text-slate-300">Logs</h3>
        <div class="flex gap-2 items-center">
          <select
            v-model="stream.logFilter"
            class="px-2 py-1 text-xs bg-black/30 border border-white/10 rounded text-slate-300 outline-none"
          >
            <option value="all">All Levels</option>
            <option value="info">Info</option>
            <option value="success">Success</option>
            <option value="warning">Warning</option>
            <option value="error">Error</option>
            <option value="debug">Debug</option>
            <option value="trace">Trace</option>
          </select>
          <button
            @click="stream.clearLogs"
            class="px-3 py-1 text-xs bg-white/10 border-0 rounded text-slate-300 hover:bg-white/20 cursor-pointer transition-colors"
          >Clear</button>
        </div>
      </div>

      <!-- Log Entries -->
      <div ref="logContainer" class="h-[300px] overflow-y-auto hide-scrollbar space-y-0.5">
        <div
          v-for="(log, i) in stream.filteredLogs"
          :key="i"
          :class="[
            'px-2 py-1 rounded text-xs font-mono flex items-start gap-2',
            logColors[log.level] || 'bg-white/5 text-slate-300'
          ]"
        >
          <span class="text-white/40 shrink-0">[{{ log.timestamp }}]</span>
          <span>{{ log.message }}</span>
        </div>
        <div v-if="stream.filteredLogs.length === 0" class="flex items-center justify-center h-full text-slate-500 text-sm">
          No logs to display
        </div>
      </div>
    </section>
  </div>
</template>

<script setup>
import { ref, watch, nextTick } from "vue";
import { useStreamStore } from "../../stores/stream.js";

const stream = useStreamStore();
const logContainer = ref(null);

const logColors = {
  info: "bg-blue-500/10 text-blue-300",
  success: "bg-emerald-500/10 text-emerald-300",
  warning: "bg-amber-500/10 text-amber-300",
  error: "bg-red-500/10 text-red-300",
  debug: "bg-purple-500/10 text-purple-300",
  trace: "bg-white/5 text-slate-400",
};

// Auto-scroll on new logs
watch(
  () => stream.filteredLogs.length,
  async () => {
    await nextTick();
    if (logContainer.value) {
      logContainer.value.scrollTop = logContainer.value.scrollHeight;
    }
  }
);
</script>
