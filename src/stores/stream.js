import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { invoke, listen } from "../composables/useTauri.js";
import { useSettingsStore } from "./settings.js";

export const useStreamStore = defineStore("stream", () => {
  const isStreaming = ref(false);
  const statusText = ref("Ready");

  // Stats
  const uptimeSeconds = ref(0);
  const bytesSent = ref(0);
  const bitrateKbps = ref(0);

  // Quality
  const qualityScore = ref(0);
  const jitter = ref(0);
  const avgLatency = ref(0);
  const bufferHealth = ref(0);
  const errorCount = ref(0);
  const bufferMs = ref(0);

  // Logs
  const logs = ref([]);
  const logFilter = ref("all");
  const MAX_LOGS = 150;

  // Toasts
  const toasts = ref([]);
  let toastId = 0;

  // Stream URLs (server mode)
  const tcpUrl = ref("");
  const httpUrl = ref("");
  const snapcastConfig = ref("");

  // Quality warning tracking
  let qualityWarningShown = false;

  // ── Computed ──
  const filteredLogs = computed(() => {
    if (logFilter.value === "all") return logs.value;
    return logs.value.filter((l) => l.level === logFilter.value);
  });

  const formattedUptime = computed(() => {
    const s = uptimeSeconds.value;
    const hrs = Math.floor(s / 3600);
    const mins = Math.floor((s % 3600) / 60);
    const secs = s % 60;
    if (hrs > 0) return `${hrs}:${String(mins).padStart(2, "0")}:${String(secs).padStart(2, "0")}`;
    return `${mins}:${String(secs).padStart(2, "0")}`;
  });

  const formattedBytes = computed(() => {
    const b = bytesSent.value;
    if (b === 0) return "0 MB";
    const mb = b / (1024 * 1024);
    if (mb < 1) return (b / 1024).toFixed(1) + " KB";
    return mb.toFixed(2) + " MB";
  });

  const qualityLabel = computed(() => {
    const s = qualityScore.value;
    if (s >= 90) return { text: "Excellent", color: "#10b981" };
    if (s >= 70) return { text: "Good", color: "#f59e0b" };
    if (s >= 50) return { text: "Fair", color: "#f97316" };
    return { text: "Poor", color: "#ef4444" };
  });

  // ── Actions ──
  function addToast(message, type = "success") {
    const id = ++toastId;
    toasts.value.push({ id, message, type });
    setTimeout(() => {
      toasts.value = toasts.value.filter((t) => t.id !== id);
    }, 3000);
  }

  function clearLogs() {
    logs.value = [];
  }

  async function startStream() {
    const settings = useSettingsStore();
    const device = settings.deviceName;
    const isServer = settings.isServer;

    if (!device) {
      statusText.value = "Select a device";
      return;
    }
    if (!isServer && !settings.ip) {
      statusText.value = "Enter Target IP";
      return;
    }
    if (!settings.port) {
      statusText.value = "Enter Target Port";
      return;
    }

    try {
      await settings.saveSettings();
      await invoke("start_stream", {
        protocol: "tcp",
        deviceName: device,
        ip: settings.ip,
        port: settings.port,
        sampleRate: settings.sampleRate,
        bufferSize: settings.bufferSize,
        ringBufferDurationMs: settings.ringBufferDuration,
        autoReconnect: settings.autoReconnect,
        highPriority: settings.highPriority,
        dscpStrategy: settings.dscpStrategy,
        chunkSize: settings.chunkSize,
        isLoopback: settings.isLoopback,
        isServer: isServer,
        enableAdaptiveBuffer: settings.adaptiveBuffer,
        minBufferMs: settings.minBuffer,
        maxBufferMs: settings.maxBuffer,
        format: settings.format,
      });

      isStreaming.value = true;
      statusText.value = isServer ? "Listening" : `Streaming to ${settings.ip}`;
      bufferMs.value = settings.ringBufferDuration;

      if (isServer) {
        const lip = settings.localIp;
        const p = settings.port;
        tcpUrl.value = `tcp://${lip}:${p}`;
        httpUrl.value = `http://${lip}:${p}/stream.wav`;
        snapcastConfig.value = `[stream]\nsource = tcp://${lip}:${p}?name=TCPStreamer&mode=client`;
      }
    } catch (error) {
      statusText.value = "Error: " + error;
    }
  }

  async function stopStream() {
    try {
      await invoke("stop_stream");
      isStreaming.value = false;
      statusText.value = "Ready";
      tcpUrl.value = "";
      httpUrl.value = "";
      snapcastConfig.value = "";
    } catch (error) {
      statusText.value = "Error stopping: " + error;
    }
  }

  async function toggleStream() {
    if (isStreaming.value) {
      await stopStream();
    } else {
      await startStream();
    }
  }

  // ── Event Listeners ──
  async function initListeners() {
    await listen("log-event", (event) => {
      const log = event.payload;
      logs.value.push(log);
      if (logs.value.length > MAX_LOGS) {
        logs.value = logs.value.slice(-MAX_LOGS);
      }
    });

    await listen("stats-event", (event) => {
      const s = event.payload;
      uptimeSeconds.value = s.uptime_seconds;
      bytesSent.value = s.bytes_sent;
      bitrateKbps.value = s.bitrate_kbps;
    });

    await listen("quality-event", (event) => {
      const q = event.payload;
      qualityScore.value = q.score;
      jitter.value = q.jitter;
      avgLatency.value = q.avg_latency;
      bufferHealth.value = q.buffer_health;
      errorCount.value = q.error_count;

      // Quality warnings
      if (q.score < 50 && !qualityWarningShown) {
        const msg = `Network quality degraded to ${q.score}. Jitter: ${q.jitter.toFixed(1)}ms`;
        addToast(msg, "warning");
        qualityWarningShown = true;
      } else if (q.score >= 70 && qualityWarningShown) {
        addToast(`Network quality recovered to ${q.score}`, "success");
        qualityWarningShown = false;
      }
    });

    await listen("buffer-resize", (event) => {
      bufferMs.value = event.payload.new_size_ms;
      logs.value.push({
        timestamp: new Date().toLocaleTimeString(),
        level: "info",
        message: `Buffer resized to ${event.payload.new_size_ms}ms: ${event.payload.reason}`,
      });
    });
  }

  return {
    // State
    isStreaming, statusText,
    uptimeSeconds, bytesSent, bitrateKbps,
    qualityScore, jitter, avgLatency, bufferHealth, errorCount, bufferMs,
    logs, logFilter, toasts,
    tcpUrl, httpUrl, snapcastConfig,
    // Computed
    filteredLogs, formattedUptime, formattedBytes, qualityLabel,
    // Actions
    addToast, clearLogs,
    startStream, stopStream, toggleStream,
    initListeners,
  };
});
