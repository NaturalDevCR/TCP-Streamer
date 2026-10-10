import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { invoke, listen } from "../composables/useTauri";
import { useSettingsStore, clampFixedLatency } from "./settings";
import { i18n } from "../i18n";
import type { LogEntry, Toast, QualityEvent, StatsEvent, BufferResizeEvent } from "../types/events";

export interface EffectiveConfig {
  role: string;
  transport: string;
  profile: string;
  sample_rate: number;
  channels: number;
  device_sample_rate: number;
  device_channels: number;
  ring_ms: number;
  target_ms: number;
  chunk_frames: number;
  capture_buffer_frames: number | null;
  adaptive: boolean;
}
export interface SessionStatus {
  effective?: EffectiveConfig | null;
  state: "running" | "starting" | "waiting" | "reconnecting" | "stopped" | "error";
  message: string;
  session_id: number;
}

export const useStreamStore = defineStore("stream", () => {
  const isStreaming = ref(false);
  const sessionState = ref<SessionStatus["state"]>("stopped");
  const sessionId = ref(0);
  const pending = ref(false);
  const stopping = ref(false);
  let operationId = 0;
  const statusDetail = ref("");
  const statusText = computed(() =>
    stopping.value
      ? i18n.global.t("session.stopping")
      : statusDetail.value || i18n.global.t(`session.${sessionState.value}`),
  );
  const statusTone = computed<"success" | "danger" | "warning" | "neutral">(() => {
    if (sessionState.value === "error") return "danger";
    if (stopping.value || ["starting", "waiting", "reconnecting"].includes(sessionState.value))
      return "warning";
    return sessionState.value === "running" ? "success" : "neutral";
  });
  let terminalSession = -1;
  const sinkMetrics = ref({ authFailures: 0, lostFrames: 0, trimPpm: 0, received: false });
  const effectiveFormat = ref<{
    sample_rate?: number;
    channels?: number;
    buffer_ms?: number;
    target_ms?: number;
  }>({});
  const effectiveConfig = ref<EffectiveConfig | null>(null);
  const activeSettings = ref<Record<string, unknown> | null>(null);
  function applySession(status: SessionStatus) {
    if (
      !status ||
      (stopping.value &&
        ["running", "starting", "waiting", "reconnecting"].includes(status.state)) ||
      status.session_id < sessionId.value ||
      (status.session_id <= terminalSession &&
        ["running", "starting", "waiting", "reconnecting"].includes(status.state))
    )
      return;
    if (["stopped", "error"].includes(status.state))
      terminalSession = Math.max(terminalSession, status.session_id);
    if (status.session_id !== sessionId.value) effectiveConfig.value = null;
    if (status.effective !== undefined) effectiveConfig.value = status.effective;
    sessionId.value = status.session_id;
    sessionState.value = status.state;
    if (status.state !== "running") qualityAvailable.value = false;
    statusDetail.value = status.state === "error" ? status.message : "";
    isStreaming.value = ["running", "starting", "waiting", "reconnecting"].includes(status.state);
    useSettingsStore().configurationLocked = isStreaming.value || pending.value;
    if (!isStreaming.value) {
      tcpUrl.value = "";
      httpUrl.value = "";
      connectionInfo.value = "";
    }
  }
  function resetMetrics() {
    effectiveFormat.value = {};
    effectiveConfig.value = null;
    statsAvailable.value = false;
    qualityAvailable.value = false;
    uptimeSeconds.value = 0;
    bytesSent.value = 0;
    bitrateKbps.value = 0;
    qualityScore.value = 0;
    rttMs.value = null;
    rttVarMs.value = null;
    underruns.value = 0;
    dropped.value = 0;
    bufferHealth.value = 0;
    bufferMs.value = 0;
    qualityWarningShown = false;
  }
  function fail(error: unknown) {
    statusDetail.value = i18n.global.t("session.failure", { error: String(error) });
    sessionState.value = "error";
    addToast(statusDetail.value, "error");
  }

  // Stats
  const uptimeSeconds = ref(0);
  const bytesSent = ref(0);
  const bitrateKbps = ref(0);

  // Quality
  const qualityUnits = ref<"events/frames" | "frames/frames">("events/frames");
  const qualityAvailable = ref(false);
  const statsAvailable = ref(false);
  const qualityScore = ref(0);
  const rttMs = ref<number | null>(null);
  const rttVarMs = ref<number | null>(null);
  const underruns = ref(0);
  const dropped = ref(0);
  const bufferHealth = ref(0);
  const bufferMs = ref(0);

  // Logs
  const logs = ref<LogEntry[]>([]);
  const logFilter = ref("all");
  const MAX_LOGS = 150;

  // Toasts
  const toasts = ref<Toast[]>([]);
  let toastId = 0;

  // Stream URLs (server mode)
  const tcpUrl = ref("");
  const httpUrl = ref("");
  const connectionInfo = ref("");

  // Quality warning tracking
  let qualityWarningShown = false;

  // ── Computed ──
  const filteredLogs = computed(() => {
    if (logFilter.value === "all") return logs.value;
    return logs.value.filter((l: LogEntry) => l.level === logFilter.value);
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
    if (s >= 90) return { text: i18n.global.t("quality.excellent"), color: "#10b981" };
    if (s >= 70) return { text: i18n.global.t("quality.good"), color: "#f59e0b" };
    if (s >= 50) return { text: i18n.global.t("quality.fair"), color: "#f97316" };
    return { text: i18n.global.t("quality.poor"), color: "#ef4444" };
  });

  // ── Actions ──
  function addToast(message: string, type: Toast["type"] = "success") {
    const id = ++toastId;
    toasts.value.push({ id, message, type });
    setTimeout(() => {
      toasts.value = toasts.value.filter((t: Toast) => t.id !== id);
    }, 3000);
  }

  function appendLog(log: LogEntry) {
    logs.value = [...logs.value.slice(-(MAX_LOGS - 1)), log];
  }

  function clearLogs() {
    logs.value = [];
  }

  async function startStream() {
    if (pending.value || isStreaming.value) return;
    const settings = useSettingsStore();
    if (settings.preflightLoading) return;
    const device = settings.deviceName;
    const isServer = settings.isServer;
    const invalid =
      settings.role === "sink"
        ? !settings.outputDevice
          ? "outputDevice"
          : !settings.sourceAddr
            ? "sourceAddress"
            : ""
        : !device
          ? "device"
          : settings.transport === "tcp" && !isServer && !settings.ip
            ? "target"
            : !Number.isInteger(settings.port) || settings.port < 1 || settings.port > 65535
              ? "port"
              : "";
    if (invalid) {
      statusDetail.value = i18n.global.t(`session.validation.${invalid}`);
      addToast(statusDetail.value, "error");
      return;
    }
    const operation = ++operationId;
    pending.value = true;
    settings.configurationLocked = true;
    sessionState.value = "starting";
    statusDetail.value = "";
    resetMetrics();
    try {
      if (!(await settings.saveSettings())) throw new Error(settings.persistenceError);
      if (operation !== operationId) return;
      let status: SessionStatus;
      if (settings.role === "sink")
        status = (await invoke("start_sink", {
          outputDevice: settings.outputDevice,
          sourceAddr: settings.sourceAddr,
          latencyProfile: settings.latencyProfile,
          overrides: {
            ringMs: settings.ringBufferDuration,
            minBufferMs: settings.minBuffer,
            maxBufferMs: settings.maxBuffer,
            chunkSize: settings.chunkSize,
            fixedLatencyMs: clampFixedLatency(settings.fixedLatencyMs),
          },
          psk: settings.psk,
        })) as SessionStatus;
      else
        status = (await invoke("start_stream", {
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
          isServer: isServer && settings.transport === "tcp",
          enableAdaptiveBuffer: settings.adaptiveBuffer,
          minBufferMs: settings.minBuffer,
          maxBufferMs: settings.maxBuffer,
          fixedLatencyMs: clampFixedLatency(settings.fixedLatencyMs),
          format: settings.transport === "udp" ? "pcm" : settings.format,
          latencyProfile: settings.latencyProfile,
          allowlist: settings.allowlist,
          transport: settings.transport,
          psk: settings.psk,
        })) as SessionStatus;
      if (operation !== operationId) return;
      applySession(status);
      if (isStreaming.value) {
        activeSettings.value = { ...settings.snapshot(), psk: undefined };
        bufferMs.value = settings.ringBufferDuration;
        if (settings.role === "source" && settings.transport === "tcp" && isServer) {
          const host = settings.localIp.includes(":") ? `[${settings.localIp}]` : settings.localIp;
          tcpUrl.value = `tcp://${host}:${settings.port}`;
          httpUrl.value = `http://${host}:${settings.port}/stream.wav`;
          connectionInfo.value = tcpUrl.value;
        }
      }
    } catch (error) {
      if (operation === operationId) {
        fail(error);
        isStreaming.value = false;
      }
    } finally {
      if (operation === operationId) {
        pending.value = false;
        settings.configurationLocked = isStreaming.value;
      }
    }
  }

  async function stopStream() {
    if (stopping.value) return;
    ++operationId;
    stopping.value = true;
    pending.value = true;
    try {
      const status = (await invoke("stop_stream")) as SessionStatus;
      if (status) applySession(status);
      else applySession({ state: "stopped", message: "", session_id: sessionId.value });
    } catch (error) {
      fail(error);
    } finally {
      pending.value = false;
      stopping.value = false;
      useSettingsStore().configurationLocked = isStreaming.value;
    }
  }

  async function toggleStream() {
    if (pending.value || isStreaming.value) {
      await stopStream();
    } else {
      await startStream();
    }
  }

  // ── Event Listeners ──
  let unlisteners: (() => void)[] = [];
  function disposeListeners() {
    unlisteners.forEach((fn) => fn());
    unlisteners = [];
  }
  async function subscribe<T>(name: string, callback: (event: { payload: T }) => void) {
    unlisteners.push(await listen(name, callback));
  }
  async function initListeners() {
    disposeListeners();
    await subscribe<SessionStatus>("stream-state", (event) => applySession(event.payload));
    try {
      applySession((await invoke("get_stream_state")) as SessionStatus);
    } catch (e) {
      fail(e);
    }

    await subscribe("log-event", (event: { payload: LogEntry }) => {
      const log = event.payload;
      appendLog(log);
    });

    await subscribe("stats-event", (event: { payload: StatsEvent }) => {
      if (
        !isStreaming.value ||
        (event.payload.session_id !== undefined && event.payload.session_id !== sessionId.value)
      )
        return;
      statsAvailable.value = true;
      const s = event.payload;
      sinkMetrics.value = {
        authFailures: s.auth_failures || 0,
        lostFrames: s.lost_frames || 0,
        trimPpm: s.trim_ppm || 0,
        received: Boolean(s.received),
      };
      effectiveFormat.value = {
        sample_rate: s.sample_rate,
        channels: s.channels,
        buffer_ms: s.buffer_ms,
        target_ms: s.target_ms,
      };
      uptimeSeconds.value = s.uptime_seconds;
      bytesSent.value = s.bytes_sent;
      bitrateKbps.value = s.bitrate_kbps;
    });

    await subscribe("quality-event", (event: { payload: QualityEvent }) => {
      if (
        !isStreaming.value ||
        (event.payload.session_id !== undefined && event.payload.session_id !== sessionId.value)
      )
        return;
      qualityAvailable.value = sessionState.value === "running";
      qualityUnits.value =
        event.payload.counter_unit ||
        (effectiveConfig.value?.role === "sink" ? "frames/frames" : "events/frames");
      const q = event.payload;
      qualityScore.value = q.score;
      rttMs.value = q.rtt_ms;
      rttVarMs.value = q.rtt_var_ms;
      underruns.value = q.underruns;
      dropped.value = q.dropped;
      bufferHealth.value = q.buffer_health;

      // Quality warnings
      if (qualityAvailable.value && q.score < 50 && !qualityWarningShown) {
        addToast(i18n.global.t("quality.degraded", { score: q.score }), "warning");
        qualityWarningShown = true;
      } else if (qualityAvailable.value && q.score >= 70 && qualityWarningShown) {
        addToast(i18n.global.t("quality.recovered", { score: q.score }), "success");
        qualityWarningShown = false;
      }
    });

    await subscribe("buffer-resize", (event: { payload: BufferResizeEvent }) => {
      if (
        !isStreaming.value ||
        (event.payload.session_id !== undefined && event.payload.session_id !== sessionId.value)
      )
        return;
      bufferMs.value = event.payload.new_size_ms;
      logs.value.push({
        timestamp: new Date().toLocaleTimeString(),
        level: "info",
        message: i18n.global.t("operation.bufferResized", {
          ms: event.payload.new_size_ms,
          reason: event.payload.reason,
        }),
        visible: true,
      });
    });
  }

  return {
    sinkMetrics,
    effectiveConfig,
    effectiveFormat,
    statsAvailable,
    qualityUnits,
    qualityAvailable,
    sessionState,
    sessionId,
    pending,
    stopping,
    activeSettings,
    applySession,
    disposeListeners,
    // State
    isStreaming,
    statusText,
    statusTone,
    uptimeSeconds,
    bytesSent,
    bitrateKbps,
    qualityScore,
    rttMs,
    rttVarMs,
    underruns,
    dropped,
    bufferHealth,
    bufferMs,
    logs,
    logFilter,
    toasts,
    tcpUrl,
    httpUrl,
    connectionInfo,
    // Computed
    filteredLogs,
    formattedUptime,
    formattedBytes,
    qualityLabel,
    // Actions
    addToast,
    clearLogs,
    startStream,
    stopStream,
    toggleStream,
    initListeners,
  };
});
