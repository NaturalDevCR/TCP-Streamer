import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { getStore } from "../composables/useTauri";
import { invoke, isEnabled, enable, disable } from "../composables/useTauri";

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
  allowlist?: string;
  mode?: string;
  role?: string;
  transport?: string;
  output_device?: string;
  source_addr?: string;
}

export const useSettingsStore = defineStore("settings", () => {
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

  // Audio
  const sampleRate = ref(48000);
  const bufferSize = ref(1024);
  const ringBufferDuration = ref(4000);
  const format = ref("pcm");

  // Adaptive buffer
  const adaptiveBuffer = ref(false);
  const minBuffer = ref(2000);
  const maxBuffer = ref(10000);

  // Automation
  const autostart = ref(false);
  const autoStream = ref(false);
  const autoReconnect = ref(false);

  // Advanced
  const highPriority = ref(true);
  const dscpStrategy = ref("voip");
  const chunkSize = ref(512);
  const latencyProfile = ref("balanced");

  // Profiles
  const profiles = ref<Record<string, SettingsDict>>({});
  const currentProfile = ref("Default");

  // Loading & Error states
  const devicesLoading = ref(false);
  const devicesError = ref("");

  // ── Computed ──
  const isServer = computed(() => mode.value === "server");
  const isLoopback = computed(() => deviceName.value.startsWith("[Loopback]"));
  const showLoopback = computed(() => osType.value === "windows");

  // ── Actions ──
  async function init() {
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

    await loadDevices();
    await loadSettings();

    try {
      autostart.value = await isEnabled();
    } catch {
      /* ignore */
    }
  }

  async function loadDevices() {
    devicesLoading.value = true;
    devicesError.value = "";
    try {
      const result = (await invoke("get_input_devices", {
        includeLoopback: loopbackMode.value,
      })) as string[];
      devices.value = result || [];
      // Restore saved device
      const store = await getStore();
      const savedDevice = (await store.get("device")) as string | null;
      if (savedDevice && devices.value.includes(savedDevice)) {
        deviceName.value = savedDevice;
      } else if (devices.value.length > 0) {
        deviceName.value = devices.value[0];
      }
    } catch (e) {
      // eslint-disable-next-line no-console
      console.error("Failed to load devices:", e);
      devices.value = [];
      devicesError.value = e instanceof Error ? e.message : "Failed to load devices";
    } finally {
      devicesLoading.value = false;
    }
  }

  async function loadOutputDevices() {
    try {
      const result = (await invoke("get_output_devices")) as string[];
      outputDevices.value = result || [];
      if (outputDevice.value && !outputDevices.value.includes(outputDevice.value)) {
        outputDevice.value = outputDevices.value[0] || "";
      } else if (!outputDevice.value && outputDevices.value.length > 0) {
        outputDevice.value = outputDevices.value[0];
      }
    } catch {
      outputDevices.value = [];
    }
  }

  async function loadSettings() {
    const store = await getStore();
    await loadProfiles();

    const profileName = ((await store.get("current_profile")) as string) || "Default";
    const allProfiles = ((await store.get("profiles")) as Record<string, SettingsDict>) || {};
    const s: SettingsDict = allProfiles[profileName] || {};

    currentProfile.value = profileName;

    if (s.ip) ip.value = s.ip as string;
    if (s.port) port.value = s.port as number;
    if (s.sample_rate) sampleRate.value = s.sample_rate as number;
    if (s.buffer_size) bufferSize.value = s.buffer_size as number;
    if (s.ring_buffer_duration) ringBufferDuration.value = s.ring_buffer_duration as number;
    if (s.format) format.value = s.format as string;
    if (s.auto_stream !== undefined) autoStream.value = s.auto_stream as boolean;
    if (s.auto_reconnect !== undefined) autoReconnect.value = s.auto_reconnect as boolean;
    if (s.high_priority !== undefined) highPriority.value = s.high_priority as boolean;
    if (s.dscp_strategy) dscpStrategy.value = s.dscp_strategy as string;
    if (s.chunk_size) chunkSize.value = s.chunk_size as number;
    if (s.adaptive_buffer !== undefined) adaptiveBuffer.value = s.adaptive_buffer as boolean;
    if (s.min_buffer) minBuffer.value = s.min_buffer as number;
    if (s.max_buffer) maxBuffer.value = s.max_buffer as number;
    if (s.latency_profile) latencyProfile.value = s.latency_profile as string;
    if (s.allowlist) allowlist.value = s.allowlist as string;
    if (s.mode) mode.value = s.mode as string;
    if (s.role) role.value = s.role as string;
    if (s.transport) transport.value = s.transport as string;
    if (s.output_device) outputDevice.value = s.output_device as string;
    if (s.source_addr) sourceAddr.value = s.source_addr as string;

    // Load loopback mode
    const savedLoopback = (await store.get("loopback_mode")) as boolean | null;
    if (savedLoopback !== null && savedLoopback !== undefined) loopbackMode.value = savedLoopback;
  }

  async function saveSettings() {
    const store = await getStore();
    const profileName = currentProfile.value || "Default";

    const settings: SettingsDict = {
      protocol: "tcp",
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
      allowlist: allowlist.value,
      mode: mode.value,
      role: role.value,
      transport: transport.value,
      output_device: outputDevice.value,
      source_addr: sourceAddr.value,
    };

    const allProfiles = ((await store.get("profiles")) as Record<string, SettingsDict>) || {};
    allProfiles[profileName] = settings;

    await store.set("profiles", allProfiles ?? {});
    await store.set("current_profile", profileName ?? "Default");
    await store.set("device", deviceName.value ?? "");
    await store.set("loopback_mode", loopbackMode.value ?? false);
    await store.save();
  }

  // ── Profiles ──
  async function loadProfiles() {
    const store = await getStore();
    const p = (await store.get("profiles")) as Record<string, SettingsDict> | null;
    if (!p || Object.keys(p).length === 0) {
      await store.set("profiles", { Default: {} });
      await store.set("current_profile", "Default");
      await store.save();
      profiles.value = { Default: {} };
    } else {
      profiles.value = p;
    }
    currentProfile.value = ((await store.get("current_profile")) as string) || "Default";
  }

  async function switchProfile(name: string) {
    currentProfile.value = name;
    const store = await getStore();
    await store.set("current_profile", name);
    await store.save();
    await loadSettings();
  }

  async function createProfile(name: string) {
    if (!name || profiles.value[name]) return false;
    const store = await getStore();
    const allProfiles = ((await store.get("profiles")) as Record<string, SettingsDict>) || {};
    allProfiles[name] = allProfiles[currentProfile.value] || {};
    await store.set("profiles", allProfiles);
    await store.set("current_profile", name);
    await store.save();
    profiles.value = allProfiles;
    currentProfile.value = name;
    return true;
  }

  async function deleteProfile(name: string) {
    if (name === "Default") return false;
    const store = await getStore();
    const allProfiles = ((await store.get("profiles")) as Record<string, SettingsDict>) || {};
    delete allProfiles[name];
    await store.set("profiles", allProfiles);
    await store.set("current_profile", "Default");
    await store.save();
    profiles.value = allProfiles;
    currentProfile.value = "Default";
    await loadSettings();
    return true;
  }

  async function toggleAutostart() {
    try {
      if (autostart.value) {
        await enable();
      } else {
        await disable();
      }
    } catch (e) {
      // eslint-disable-next-line no-console
      console.error("Autostart error:", e);
    }
  }

  return {
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
    autoStream,
    autoReconnect,
    highPriority,
    dscpStrategy,
    chunkSize,
    latencyProfile,
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
    toggleAutostart,
    loadOutputDevices,
  };
});
