import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { getStore, isNativeRuntime } from "../composables/useTauri";
import { invoke, isEnabled, enable, disable } from "../composables/useTauri";

export interface PreflightResult {
  ready: boolean;
  stages: { name: string; status: "ok" | "error" | "warning"; detail: string }[];
}

interface SettingsDict {
  [key: string]: unknown;
  protocol?: string;
  device?: string;
  ip?: string;
  port?: number;
  sample_rate?: number;
  buffer_size?: number;
  ring_buffer_duration?: number;
  format?: string;
  auto_stream?: boolean;
  auto_reconnect?: boolean;
  high_priority?: boolean;
  dscp_strategy?: string;
  chunk_size?: number;
  adaptive_buffer?: boolean;
  min_buffer?: number;
  max_buffer?: number;
  network_preset?: string;
  latency_profile?: string;
  fixed_latency_ms?: number;
  allowlist?: string;
  mode?: string;
  role?: string;
  transport?: string;
  output_device?: string;
  source_addr?: string;
  psk?: string;
}

/** Bounds mirror FIXED_LATENCY_MIN_MS / FIXED_LATENCY_MAX_MS in latency.rs. */
export const FIXED_LATENCY_MIN_MS = 50;
export const FIXED_LATENCY_MAX_MS = 2000;

/**
 * Clamps a fixed-latency value into the range the backend accepts. A number
 * input yields 0 for an empty field and NaN for a stray "-" or "e", and either
 * would be silently discarded on the next load — so nothing invalid is ever
 * persisted or sent.
 */
export function clampFixedLatency(value: number): number {
  if (!Number.isFinite(value)) return 250;
  return Math.min(Math.max(Math.round(value), FIXED_LATENCY_MIN_MS), FIXED_LATENCY_MAX_MS);
}

export const useSettingsStore = defineStore("settings", () => {
  const nativeAvailable = isNativeRuntime();
  // ── Reactive State ──
  const devices = ref<string[]>([]);
  const osType = ref("unknown");
  const localIp = ref("127.0.0.1");

  // Role & Transport
  const role = ref("source"); // "source" | "sink"
  const transport = ref("tcp"); // "tcp" | "udp"

  // Connection
  const deviceName = ref("");
  const mode = ref("client"); // "client" | "server"
  const ip = ref("");
  const port = ref(1704);
  const loopbackMode = ref(false);
  const allowlist = ref("");

  // Sink
  const outputDevice = ref("");
  const sourceAddr = ref("");
  const outputDevices = ref<string[]>([]);

  // Encryption
  const psk = ref("");
  const rememberPsk = ref(false);

  // mDNS discovery
  const discoveredSources = ref<{ name: string; addr: string; encrypted: boolean }[]>([]);

  // Audio
  const sampleRate = ref(48000);
  const bufferSize = ref(1024);
  const ringBufferDuration = ref(4000);
  const format = ref("pcm");

  // Adaptive buffer
  const adaptiveBuffer = ref(false);
  const minBuffer = ref(200);
  const maxBuffer = ref(1500);

  // Automation
  const autostart = ref(false);
  const autostartBusy = ref(false);
  const autoStream = ref(false);
  const autoReconnect = ref(false);

  // Advanced
  const highPriority = ref(true);
  const dscpStrategy = ref("voip");
  const chunkSize = ref(512);
  const latencyProfile = ref("balanced");
  const fixedLatencyMs = ref(250);

  // Profiles
  const profiles = ref<Record<string, SettingsDict>>({});
  const currentProfile = ref("Default");

  // Loading & Error states
  const devicesLoading = ref(false);
  const devicesError = ref("");

  const preflightResult = ref<PreflightResult | null>(null);
  const preflightLoading = ref(false);
  const preflightError = ref("");
  const preflightSnapshot = ref("");
  const preflightStale = computed(() => preflightSnapshot.value !== JSON.stringify(snapshot()));
  const persistenceError = ref("");
  const outputDevicesLoading = ref(false);
  const outputDevicesError = ref("");
  const inputDevicesKnown = ref(false);
  const outputDevicesKnown = ref(false);
  const inputUnavailable = computed(
    () =>
      inputDevicesKnown.value && !!deviceName.value && !devices.value.includes(deviceName.value),
  );
  const outputUnavailable = computed(
    () =>
      outputDevicesKnown.value &&
      !!outputDevice.value &&
      !outputDevices.value.includes(outputDevice.value),
  );
  const scanLoading = ref(false);
  const scanError = ref("");
  const scanCompleted = ref(false);
  const configurationLocked = ref(false);
  const profileBusy = ref(false);
  const initializing = ref(false);
  const savedSnapshot = ref("");
  const isDirty = computed(() => JSON.stringify(snapshot()) !== savedSnapshot.value);
  function snapshot(): SettingsDict {
    return {
      device: deviceName.value,
      ip: ip.value,
      port: port.value,
      sample_rate: sampleRate.value,
      buffer_size: bufferSize.value,
      ring_buffer_duration: ringBufferDuration.value,
      format: format.value,
      auto_stream: autoStream.value,
      auto_reconnect: autoReconnect.value,
      high_priority: highPriority.value,
      dscp_strategy: dscpStrategy.value,
      chunk_size: chunkSize.value,
      adaptive_buffer: adaptiveBuffer.value,
      min_buffer: minBuffer.value,
      max_buffer: maxBuffer.value,
      latency_profile: latencyProfile.value,
      fixed_latency_ms: fixedLatencyMs.value,
      allowlist: allowlist.value,
      mode: mode.value,
      role: role.value,
      transport: transport.value,
      output_device: outputDevice.value,
      source_addr: sourceAddr.value,
      psk: psk.value,
      remember_psk: rememberPsk.value,
      loopback_mode: loopbackMode.value,
    };
  }
  function applyProfile(raw: SettingsDict) {
    deviceName.value = typeof raw.device === "string" ? raw.device : "";
    ip.value = typeof raw.ip === "string" ? raw.ip : "";
    port.value = typeof raw.port === "number" && Number.isFinite(raw.port) ? raw.port : 1704;
    sampleRate.value =
      typeof raw.sample_rate === "number" && Number.isFinite(raw.sample_rate)
        ? raw.sample_rate
        : 48000;
    bufferSize.value =
      typeof raw.buffer_size === "number" && Number.isFinite(raw.buffer_size)
        ? raw.buffer_size
        : 1024;
    ringBufferDuration.value =
      typeof raw.ring_buffer_duration === "number" && Number.isFinite(raw.ring_buffer_duration)
        ? raw.ring_buffer_duration
        : 4000;
    format.value = typeof raw.format === "string" ? raw.format : "pcm";
    autoStream.value = typeof raw.auto_stream === "boolean" ? raw.auto_stream : false;
    autoReconnect.value = typeof raw.auto_reconnect === "boolean" ? raw.auto_reconnect : false;
    highPriority.value = typeof raw.high_priority === "boolean" ? raw.high_priority : true;
    dscpStrategy.value = typeof raw.dscp_strategy === "string" ? raw.dscp_strategy : "voip";
    chunkSize.value =
      typeof raw.chunk_size === "number" && Number.isFinite(raw.chunk_size) ? raw.chunk_size : 512;
    adaptiveBuffer.value = typeof raw.adaptive_buffer === "boolean" ? raw.adaptive_buffer : false;
    minBuffer.value =
      typeof raw.min_buffer === "number" && Number.isFinite(raw.min_buffer) ? raw.min_buffer : 200;
    maxBuffer.value =
      typeof raw.max_buffer === "number" && Number.isFinite(raw.max_buffer) ? raw.max_buffer : 1500;
    latencyProfile.value =
      typeof raw.latency_profile === "string" ? raw.latency_profile : "balanced";
    fixedLatencyMs.value =
      typeof raw.fixed_latency_ms === "number" && Number.isFinite(raw.fixed_latency_ms)
        ? raw.fixed_latency_ms
        : 250;
    allowlist.value = typeof raw.allowlist === "string" ? raw.allowlist : "";
    mode.value = typeof raw.mode === "string" ? raw.mode : "client";
    role.value = typeof raw.role === "string" ? raw.role : "source";
    transport.value = typeof raw.transport === "string" ? raw.transport : "tcp";
    outputDevice.value = typeof raw.output_device === "string" ? raw.output_device : "";
    sourceAddr.value = typeof raw.source_addr === "string" ? raw.source_addr : "";
    rememberPsk.value = typeof raw.remember_psk === "boolean" ? raw.remember_psk : Boolean(raw.psk);
    psk.value = rememberPsk.value && typeof raw.psk === "string" ? raw.psk : "";
    loopbackMode.value = typeof raw.loopback_mode === "boolean" ? raw.loopback_mode : false;
    const enums = {
      mode: ["client", "server"],
      role: ["source", "sink"],
      transport: ["tcp", "udp"],
      format: ["pcm", "wav"],
      latencyProfile: ["ultra-low", "balanced", "robust", "broadcast", "custom"],
      dscpStrategy: ["voip", "ef", "cs5", "lowdelay", "throughput", "besteffort"],
    };
    if (!enums.mode.includes(mode.value)) mode.value = "client";
    if (!enums.role.includes(role.value)) role.value = "source";
    if (!enums.transport.includes(transport.value)) transport.value = "tcp";
    if (!enums.format.includes(format.value)) format.value = "pcm";
    if (!enums.latencyProfile.includes(latencyProfile.value)) latencyProfile.value = "balanced";
    if (!enums.dscpStrategy.includes(dscpStrategy.value)) dscpStrategy.value = "voip";
    fixedLatencyMs.value = clampFixedLatency(fixedLatencyMs.value);
  }
  // ── Computed ──
  const isServer = computed(() => mode.value === "server");
  const isLoopback = computed(() => deviceName.value.startsWith("[Loopback]"));
  const showLoopback = computed(() => osType.value === "windows");

  // ── Actions ──
  async function init() {
    if (!nativeAvailable) {
      // A preview can be reinitialized after a failed native startup or hot reload.
      persistenceError.value = "";
      devicesError.value = "";
      outputDevicesError.value = "";
      scanError.value = "";
      preflightError.value = "";
      currentProfile.value = "Default";
      profiles.value = { Default: {} };
      applyProfile({});
      savedSnapshot.value = JSON.stringify(snapshot());
      return;
    }
    try {
      osType.value = (await invoke("get_os_type")) as string;
    } catch {
      /* ignore */
    }

    try {
      localIp.value = (await invoke("get_local_ip")) as string;
    } catch {
      /* ignore */
    }

    try {
      await loadSettings();
    } catch (e) {
      persistenceError.value = String(e);
    }
    await Promise.all([loadDevices(), loadOutputDevices()]);

    try {
      autostart.value = await isEnabled();
    } catch {
      /* ignore */
    }
  }

  let deviceGeneration = 0;
  async function loadDevices(refresh = false) {
    const generation = ++deviceGeneration;
    devicesLoading.value = true;
    devicesError.value = "";
    try {
      const result = (await invoke("get_input_devices", {
        includeLoopback: loopbackMode.value,
        refresh,
      })) as string[];
      if (generation !== deviceGeneration) return;
      devices.value = result || [];
      inputDevicesKnown.value = true;
      if (!deviceName.value && devices.value.length) deviceName.value = devices.value[0];
    } catch (e) {
      if (generation !== deviceGeneration) return;
      devices.value = [];
      devicesError.value = String(e);
    } finally {
      if (generation === deviceGeneration) devicesLoading.value = false;
    }
  }

  async function loadOutputDevices() {
    if (outputDevicesLoading.value) return;
    outputDevicesLoading.value = true;
    outputDevicesError.value = "";
    try {
      const result = (await invoke("get_output_devices")) as string[];
      outputDevices.value = result || [];
      outputDevicesKnown.value = true;
      if (!outputDevice.value && outputDevices.value.length)
        outputDevice.value = outputDevices.value[0];
    } catch (e) {
      outputDevices.value = [];
      outputDevicesError.value = String(e);
    } finally {
      outputDevicesLoading.value = false;
    }
  }

  let scanGeneration = 0;
  function cancelScan() {
    scanGeneration++;
    scanLoading.value = false;
    scanCompleted.value = false;
  }
  async function preflight() {
    if (
      configurationLocked.value ||
      profileBusy.value ||
      initializing.value ||
      preflightLoading.value
    )
      return;
    preflightLoading.value = true;
    preflightError.value = "";
    preflightResult.value = null;
    preflightSnapshot.value = JSON.stringify(snapshot());
    const config = {
      role: role.value,
      transport: role.value === "sink" ? "udp" : transport.value,
      mode: mode.value,
      device: role.value === "sink" ? outputDevice.value : deviceName.value,
      host: ip.value,
      port: port.value,
      sourceAddr: sourceAddr.value,
      sampleRate: sampleRate.value,
      bufferSize: bufferSize.value,
      isLoopback: isLoopback.value,
      latencyProfile: latencyProfile.value,
      overrides: {
        ringMs: ringBufferDuration.value,
        minBufferMs: minBuffer.value,
        maxBufferMs: maxBuffer.value,
        chunkSize: chunkSize.value,
        fixedLatencyMs: clampFixedLatency(fixedLatencyMs.value),
      },
      allowlist: allowlist.value,
    };
    try {
      preflightResult.value = (await invoke("preflight_stream", { config })) as PreflightResult;
    } catch (e) {
      preflightError.value = String(e);
    } finally {
      preflightLoading.value = false;
    }
  }
  async function scanSources() {
    if (scanLoading.value) return;
    const generation = ++scanGeneration;
    scanLoading.value = true;
    scanError.value = "";
    scanCompleted.value = false;
    try {
      const result = (await invoke("list_sources")) as {
        name: string;
        addr: string;
        encrypted: boolean;
      }[];
      if (generation !== scanGeneration) return;
      discoveredSources.value = result;
    } catch (e) {
      if (generation !== scanGeneration) return;
      discoveredSources.value = [];
      scanError.value = String(e);
    } finally {
      if (generation === scanGeneration) {
        discoveredSources.value = [
          ...new Map(discoveredSources.value.map((s) => [s.addr, s])).values(),
        ];
        scanLoading.value = false;
        scanCompleted.value = true;
      }
    }
  }

  async function loadSettings() {
    const store = await getStore();
    const stored = (await store.get("profiles")) as Record<string, SettingsDict> | null;
    const savedName = await store.get("current_profile");
    const allProfiles: Record<string, SettingsDict> = { Default: {}, ...stored };
    const name =
      typeof savedName === "string" && Object.hasOwn(allProfiles, savedName)
        ? savedName
        : "Default";
    // The old global loopback flag belongs to the profile selected on startup.
    const legacyLoopback =
      allProfiles[name]?.loopback_mode === undefined && Boolean(await store.get("loopback_mode"));
    for (const [profileName, profile] of Object.entries(allProfiles)) {
      if (typeof profile?.loopback_mode !== "boolean") {
        allProfiles[profileName] = {
          ...profile,
          loopback_mode: profileName === name && legacyLoopback,
        };
      }
    }
    profiles.value = allProfiles;
    currentProfile.value = name;
    applyProfile(allProfiles[name]);
    savedSnapshot.value = JSON.stringify(snapshot());
  }

  async function commitProfiles(allProfiles: Record<string, SettingsDict>, name: string) {
    const store = await getStore();
    const previousProfiles = await store.get("profiles");
    const previousName = await store.get("current_profile");
    try {
      await store.set("profiles", allProfiles);
      await store.set("current_profile", name);
      await store.save();
    } catch (error) {
      // The plugin updates its in-memory store before the explicit disk write.
      await Promise.allSettled([
        store.set("profiles", previousProfiles),
        store.set("current_profile", previousName),
      ]);
      throw error;
    }
    profiles.value = allProfiles;
    currentProfile.value = name;
    persistenceError.value = "";
  }

  function profileForDisk(profile: SettingsDict): SettingsDict {
    return {
      ...profile,
      psk: profile.remember_psk ? profile.psk : "",
      fixed_latency_ms: clampFixedLatency(Number(profile.fixed_latency_ms)),
    };
  }

  async function persistSettings() {
    const draft = snapshot();
    const name = currentProfile.value;
    await commitProfiles({ ...profiles.value, [name]: profileForDisk(draft) }, name);
    savedSnapshot.value = JSON.stringify(draft);
  }

  async function withProfileOperation(action: () => Promise<boolean>) {
    if (profileBusy.value) return false;
    profileBusy.value = true;
    try {
      return await action();
    } catch (error) {
      persistenceError.value = String(error);
      throw error;
    } finally {
      profileBusy.value = false;
    }
  }

  async function saveSettings() {
    try {
      return await withProfileOperation(async () => {
        await persistSettings();
        return true;
      });
    } catch {
      return false;
    }
  }
  function exportProfile(): string {
    const profile = {
      ...snapshot(),
      remember_psk: false,
      psk: "",
      ip: "",
      source_addr: "",
      allowlist: "",
    };
    return JSON.stringify({ schema_version: 1, profile }, null, 2);
  }
  async function activateProfile(profile: SettingsDict) {
    const previousLoopback = loopbackMode.value;
    applyProfile(profile);
    savedSnapshot.value = JSON.stringify(snapshot());
    if (inputDevicesKnown.value && loopbackMode.value !== previousLoopback) await loadDevices();
    else if (!deviceName.value && devices.value.length) deviceName.value = devices.value[0];
    if (!outputDevice.value && outputDevices.value.length)
      outputDevice.value = outputDevices.value[0];
  }

  function canCreateProfile(name: string) {
    return !configurationLocked.value && !!name.trim() && !Object.hasOwn(profiles.value, name);
  }

  async function importProfile(name: string, content: string) {
    if (!canCreateProfile(name)) return false;
    return withProfileOperation(async () => {
      const data = JSON.parse(content);
      if (
        data?.schema_version !== 1 ||
        !data.profile ||
        typeof data.profile !== "object" ||
        Array.isArray(data.profile)
      )
        throw new Error("Unsupported profile schema");
      const profile = {
        ...data.profile,
        psk: "",
        remember_psk: false,
        loopback_mode: data.profile.loopback_mode === true,
      };
      const allProfiles = { ...profiles.value };
      if (isDirty.value) allProfiles[currentProfile.value] = profileForDisk(snapshot());
      await commitProfiles({ ...allProfiles, [name]: profile }, name);
      await activateProfile(profile);
      return true;
    });
  }

  async function switchProfile(name: string) {
    if (
      configurationLocked.value ||
      !Object.hasOwn(profiles.value, name) ||
      name === currentProfile.value
    )
      return;
    await withProfileOperation(async () => {
      const allProfiles = { ...profiles.value };
      if (isDirty.value) allProfiles[currentProfile.value] = profileForDisk(snapshot());
      await commitProfiles(allProfiles, name);
      await activateProfile(allProfiles[name]);
      return true;
    });
  }

  async function createProfile(name: string) {
    if (!canCreateProfile(name)) return false;
    return withProfileOperation(async () => {
      const draft = snapshot();
      await commitProfiles({ ...profiles.value, [name]: profileForDisk(draft) }, name);
      savedSnapshot.value = JSON.stringify(draft);
      return true;
    });
  }

  async function deleteProfile(name: string) {
    if (configurationLocked.value || name === "Default" || !Object.hasOwn(profiles.value, name))
      return false;
    return withProfileOperation(async () => {
      const deletingCurrent = name === currentProfile.value;
      const next = deletingCurrent ? "Default" : currentProfile.value;
      const allProfiles: Record<string, SettingsDict> = { Default: {}, ...profiles.value };
      delete allProfiles[name];
      await commitProfiles(allProfiles, next);
      if (deletingCurrent) await activateProfile(allProfiles[next]);
      return true;
    });
  }

  async function setAutostart(enabled: boolean) {
    if (autostartBusy.value || enabled === autostart.value) return;
    autostartBusy.value = true;
    try {
      if (enabled) {
        await enable();
      } else {
        await disable();
      }
      autostart.value = enabled;
    } catch (e) {
      persistenceError.value = String(e);
    } finally {
      autostartBusy.value = false;
    }
  }

  return {
    nativeAvailable,
    preflight,
    preflightResult,
    preflightLoading,
    preflightError,
    preflightStale,
    rememberPsk,
    snapshot,
    isDirty,
    configurationLocked,
    profileBusy,
    initializing,
    inputUnavailable,
    outputUnavailable,
    persistenceError,
    exportProfile,
    importProfile,
    outputDevicesLoading,
    outputDevicesError,
    scanLoading,
    scanError,
    scanCompleted,
    // State
    devices,
    osType,
    localIp,
    deviceName,
    mode,
    ip,
    port,
    loopbackMode,
    sampleRate,
    bufferSize,
    ringBufferDuration,
    format,
    adaptiveBuffer,
    minBuffer,
    maxBuffer,
    autostart,
    autostartBusy,
    autoStream,
    autoReconnect,
    highPriority,
    dscpStrategy,
    chunkSize,
    latencyProfile,
    fixedLatencyMs,
    allowlist,
    profiles,
    currentProfile,
    devicesLoading,
    devicesError,
    role,
    transport,
    outputDevice,
    sourceAddr,
    outputDevices,
    psk,
    discoveredSources,
    // Computed
    isServer,
    isLoopback,
    showLoopback,
    // Actions
    init,
    loadDevices,
    loadSettings,
    saveSettings,
    switchProfile,
    createProfile,
    deleteProfile,
    setAutostart,
    loadOutputDevices,
    scanSources,
    cancelScan,
  };
});
