import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { getStore } from "../composables/useTauri.js";
import { invoke, isEnabled, enable, disable } from "../composables/useTauri.js";

export const useSettingsStore = defineStore("settings", () => {
  // ── Reactive State ──
  const devices = ref([]);
  const osType = ref("unknown");
  const localIp = ref("127.0.0.1");

  // Connection
  const deviceName = ref("");
  const mode = ref("client"); // "client" | "server"
  const ip = ref("");
  const port = ref(1704);
  const loopbackMode = ref(false);

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
  const networkPreset = ref("custom");

  // Profiles
  const profiles = ref({});
  const currentProfile = ref("Default");

  // ── Computed ──
  const isServer = computed(() => mode.value === "server");
  const isLoopback = computed(() => deviceName.value.startsWith("[Loopback]"));
  const showLoopback = computed(() => osType.value === "windows");

  // ── Actions ──
  async function init() {
    try {
      osType.value = await invoke("get_os_type");
    } catch { /* ignore */ }

    try {
      localIp.value = await invoke("get_local_ip");
    } catch { /* ignore */ }

    await loadDevices();
    await loadSettings();

    try {
      autostart.value = await isEnabled();
    } catch { /* ignore */ }
  }

  async function loadDevices() {
    try {
      const result = await invoke("get_input_devices", { includeLoopback: loopbackMode.value });
      devices.value = result || [];
      // Restore saved device
      const store = await getStore();
      const savedDevice = await store.get("device");
      if (savedDevice && devices.value.includes(savedDevice)) {
        deviceName.value = savedDevice;
      } else if (devices.value.length > 0) {
        deviceName.value = devices.value[0];
      }
    } catch (e) {
      console.error("Failed to load devices:", e);
      devices.value = [];
    }
  }

  async function loadSettings() {
    const store = await getStore();
    await loadProfiles();

    const profileName = (await store.get("current_profile")) || "Default";
    const allProfiles = (await store.get("profiles")) || {};
    const s = allProfiles[profileName] || {};

    currentProfile.value = profileName;

    if (s.ip) ip.value = s.ip;
    if (s.port) port.value = s.port;
    if (s.sample_rate) sampleRate.value = s.sample_rate;
    if (s.buffer_size) bufferSize.value = s.buffer_size;
    if (s.ring_buffer_duration) ringBufferDuration.value = s.ring_buffer_duration;
    if (s.format) format.value = s.format;
    if (s.auto_stream !== undefined) autoStream.value = s.auto_stream;
    if (s.auto_reconnect !== undefined) autoReconnect.value = s.auto_reconnect;
    if (s.high_priority !== undefined) highPriority.value = s.high_priority;
    if (s.dscp_strategy) dscpStrategy.value = s.dscp_strategy;
    if (s.chunk_size) chunkSize.value = s.chunk_size;
    if (s.adaptive_buffer !== undefined) adaptiveBuffer.value = s.adaptive_buffer;
    if (s.min_buffer) minBuffer.value = s.min_buffer;
    if (s.max_buffer) maxBuffer.value = s.max_buffer;
    if (s.network_preset) networkPreset.value = s.network_preset;
    if (s.mode) mode.value = s.mode;

    // Load loopback mode
    const savedLoopback = await store.get("loopback_mode");
    if (savedLoopback !== null && savedLoopback !== undefined) loopbackMode.value = savedLoopback;
  }

  async function saveSettings() {
    const store = await getStore();
    const profileName = currentProfile.value || "Default";

    const settings = {
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
      network_preset: networkPreset.value,
      mode: mode.value,
    };

    const allProfiles = (await store.get("profiles")) || {};
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
    const p = await store.get("profiles");
    if (!p || Object.keys(p).length === 0) {
      await store.set("profiles", { Default: {} });
      await store.set("current_profile", "Default");
      await store.save();
      profiles.value = { Default: {} };
    } else {
      profiles.value = p;
    }
    currentProfile.value = (await store.get("current_profile")) || "Default";
  }

  async function switchProfile(name) {
    currentProfile.value = name;
    const store = await getStore();
    await store.set("current_profile", name);
    await store.save();
    await loadSettings();
  }

  async function createProfile(name) {
    if (!name || profiles.value[name]) return false;
    const store = await getStore();
    const allProfiles = (await store.get("profiles")) || {};
    allProfiles[name] = allProfiles[currentProfile.value] || {};
    await store.set("profiles", allProfiles);
    await store.set("current_profile", name);
    await store.save();
    profiles.value = allProfiles;
    currentProfile.value = name;
    return true;
  }

  async function deleteProfile(name) {
    if (name === "Default") return false;
    const store = await getStore();
    const allProfiles = (await store.get("profiles")) || {};
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
      console.error("Autostart error:", e);
    }
  }

  // ── Network Presets ──
  const PRESETS = {
    ethernet: { ring_buffer_duration: 2000, chunk_size: 512, min_buffer: 2000, max_buffer: 6000, adaptive_buffer: true },
    wifi: { ring_buffer_duration: 4000, chunk_size: 1024, min_buffer: 3000, max_buffer: 10000, adaptive_buffer: true },
    "wifi-poor": { ring_buffer_duration: 8000, chunk_size: 2048, min_buffer: 5000, max_buffer: 15000, adaptive_buffer: true },
  };

  function applyPreset(presetId) {
    if (presetId === "custom") return;
    const preset = PRESETS[presetId];
    if (!preset) return;
    ringBufferDuration.value = preset.ring_buffer_duration;
    chunkSize.value = preset.chunk_size;
    minBuffer.value = preset.min_buffer;
    maxBuffer.value = preset.max_buffer;
    adaptiveBuffer.value = preset.adaptive_buffer;
    networkPreset.value = presetId;
    saveSettings();
  }

  return {
    // State
    devices, osType, localIp,
    deviceName, mode, ip, port, loopbackMode,
    sampleRate, bufferSize, ringBufferDuration, format,
    adaptiveBuffer, minBuffer, maxBuffer,
    autostart, autoStream, autoReconnect,
    highPriority, dscpStrategy, chunkSize, networkPreset,
    profiles, currentProfile,
    // Computed
    isServer, isLoopback, showLoopback,
    // Actions
    init, loadDevices, loadSettings, saveSettings,
    switchProfile, createProfile, deleteProfile,
    toggleAutostart, applyPreset,
  };
});
