import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";
import { Store } from "@tauri-apps/plugin-store";

let store; // Store will be initialized async

// Debug Store (will log after init)

let isStreaming = false;
let deviceSelect,
  ipInput,
  portInput,
  streamNameInput,
  sampleRateSelect,
  bufferSizeSelect,
  ringBufferDurationSelect,
  autostartCheck,
  autostreamCheck,
  autoReconnectCheck,
  toggleBtn,
  statusBadge,
  statusText;
let priorityCheck, dscpSelect, chunkSizeSelect;
let silenceThreshold = 5;
let silenceTimeoutSeconds = 0; // Default to 0 (disabled) to prevent confusion
let profileSelect,
  btnSaveProfile,
  btnNewProfile,
  btnDeleteProfile,
  newProfileContainer,
  newProfileName,
  btnConfirmProfile,
  btnCancelProfile;
let logsContainer, clearLogsBtn, statsBar;
let tabBtns, tabPanes;
let loopbackMode = false;
let loopbackModeInput;

const MAX_LOGS = 100;

function updateStatus(active, text) {
  if (active) {
    statusBadge.classList.add("active");
    toggleBtn.classList.add("stop");
    toggleBtn.querySelector(".btn-text").textContent = "Stop Streaming";
    // Show stats bar when streaming
    if (statsBar) statsBar.style.display = "flex";
  } else {
    statusBadge.classList.remove("active");
    toggleBtn.classList.remove("stop");
    toggleBtn.querySelector(".btn-text").textContent = "Start Streaming";
    // Hide stats bar when not streaming
    if (statsBar) statsBar.style.display = "none";
  }
  statusText.textContent = text;
}

// Log system
// Log system
let currentLogFilter = "all";

function addLog(log) {
  const entry = document.createElement("div");
  entry.className = `log-entry log-${log.level}`;
  entry.dataset.level = log.level; // Store level for filtering
  entry.innerHTML = `<span class="log-time">[${log.timestamp}]</span> ${log.message}`;

  // Apply filter
  if (currentLogFilter !== "all" && log.level !== currentLogFilter) {
    entry.style.display = "none";
  }

  logsContainer.appendChild(entry);

  // Limit logs to MAX_LOGS
  while (logsContainer.children.length > MAX_LOGS) {
    logsContainer.removeChild(logsContainer.firstChild);
  }

  // Auto-scroll to bottom
  logsContainer.scrollTop = logsContainer.scrollHeight;
}

function filterLogs(level) {
  currentLogFilter = level;
  const entries = logsContainer.children;
  for (let entry of entries) {
    if (level === "all" || entry.dataset.level === level) {
      entry.style.display = "block";
    } else {
      entry.style.display = "none";
    }
  }
}

function clearLogs() {
  logsContainer.innerHTML = "";
}

// Toast Notifications
function showNotification(message, type = "success") {
  const container = document.getElementById("toast-container");
  if (!container) return;

  const toast = document.createElement("div");
  toast.className = `toast ${type}`;

  let icon = "";
  if (type === "success") icon = "✓";
  if (type === "error") icon = "✕";
  if (type === "info") icon = "ℹ";

  toast.innerHTML = `<span style="font-weight:bold">${icon}</span> ${message}`;

  container.appendChild(toast);

  // Remove after animation (3s total: 0.3s in + 2.4s wait + 0.3s out)
  setTimeout(() => {
    if (toast.parentNode) {
      toast.parentNode.removeChild(toast);
    }
  }, 3000);
}

// Statistics formatting
function formatUptime(seconds) {
  const hrs = Math.floor(seconds / 3600);
  const mins = Math.floor((seconds % 3600) / 60);
  const secs = seconds % 60;

  if (hrs > 0) {
    return `${hrs}:${mins.toString().padStart(2, "0")}:${secs
      .toString()
      .padStart(2, "0")}`;
  }
  return `${mins}:${secs.toString().padStart(2, "0")}`;
}

function formatBytes(bytes) {
  if (bytes === 0) return "0 MB";
  const mb = bytes / (1024 * 1024);
  if (mb < 1) {
    return (bytes / 1024).toFixed(1) + " KB";
  }
  return mb.toFixed(2) + " MB";
}

function updateStats(stats) {
  document.getElementById("stat-uptime").textContent = formatUptime(
    stats.uptime_seconds
  );
  document.getElementById("stat-bytes").textContent = formatBytes(
    stats.bytes_sent
  );
  document.getElementById("stat-bitrate").textContent =
    stats.bitrate_kbps.toFixed(1) + " kbps";
}

async function loadDevices() {
  try {
    console.log("Requesting devices...");
    // Assuming loopbackMode is defined elsewhere or will be added.
    // For now, we'll declare 'devices' with 'let' to allow reassignment.
    let devices;
    // Pass loopback mode to backend
    devices = await invoke("get_input_devices", {
      includeLoopback: loopbackMode,
    });
    console.log("Devices received:", devices);

    deviceSelect.innerHTML = "";
    if (!devices || devices.length === 0) {
      const option = document.createElement("option");
      option.text = "No devices found";
      option.disabled = true;
      deviceSelect.add(option);
      return;
    }

    devices.forEach((device) => {
      const option = document.createElement("option");
      option.value = device;
      option.text = device;
      deviceSelect.add(option);
    });

    // Restore selected device
    try {
      const savedDevice = await store.get("device");
      if (savedDevice) {
        deviceSelect.value = savedDevice;
      }
    } catch (e) {
      console.warn("Could not load saved device:", e);
    }
  } catch (error) {
    console.error("Failed to load devices:", error);
    updateStatus(false, "Error loading devices");

    // Fallback for UI if backend fails
    deviceSelect.innerHTML = "";
    const option = document.createElement("option");
    option.text = "Error: " + error;
    option.disabled = true;
    deviceSelect.add(option);
  }
}

async function loadSettings() {
  try {
    console.log("📂 Loading settings from store...");

    // Load profiles first
    await loadProfiles();

    const currentProfile = (await store.get("current_profile")) || "Default";
    const profiles = (await store.get("profiles")) || {};
    const settings = profiles[currentProfile] || {};

    console.log(`Loading profile: ${currentProfile}`, settings);

    if (settings.ip) ipInput.value = settings.ip;
    if (settings.port) portInput.value = settings.port;
    if (settings.sample_rate) sampleRateSelect.value = settings.sample_rate;
    if (settings.sample_rate) sampleRateSelect.value = settings.sample_rate;
    if (settings.buffer_size) bufferSizeSelect.value = settings.buffer_size;
    if (settings.ring_buffer_duration)
      ringBufferDurationSelect.value = settings.ring_buffer_duration;
    if (settings.auto_stream !== undefined)
      autostreamCheck.checked = settings.auto_stream;
    if (settings.auto_reconnect !== undefined)
      autoReconnectCheck.checked = settings.auto_reconnect;
    if (settings.high_priority !== undefined)
      priorityCheck.checked = settings.high_priority;
    if (settings.dscp_strategy) dscpSelect.value = settings.dscp_strategy;
    if (settings.chunk_size) chunkSizeSelect.value = settings.chunk_size;
    if (settings.silence_threshold !== undefined)
      silenceThreshold = settings.silence_threshold;
    else silenceThreshold = 5; // Default value
    if (settings.silence_timeout !== undefined)
      silenceTimeoutSeconds = settings.silence_timeout;
    else silenceTimeoutSeconds = 0; // Default value (disabled)

    // EQ and Gain removed

    // Set profile dropdown
    if (profileSelect) profileSelect.value = currentProfile;

    console.log("✅ Settings loaded successfully");
  } catch (e) {
    console.warn("❌ Failed to load settings from store:", e);
  }

  try {
    const autostart = await isEnabled();
    autostartCheck.checked = autostart;
    console.log("Autostart enabled:", autostart);
  } catch (e) {
    console.warn("Failed to check autostart status:", e);
  }
}

async function saveSettings() {
  try {
    console.log("💾 Saving settings...");
    const currentProfile = profileSelect.value || "Default";

    const settings = {
      device: deviceSelect.value,
      ip: ipInput.value,
      port: parseInt(portInput.value),
      sample_rate: parseInt(sampleRateSelect.value),
      sample_rate: parseInt(sampleRateSelect.value),
      buffer_size: parseInt(bufferSizeSelect.value),
      ring_buffer_duration: parseInt(ringBufferDurationSelect.value),
      auto_stream: autostreamCheck.checked,
      auto_reconnect: autoReconnectCheck.checked,
      high_priority: priorityCheck.checked,
      dscp_strategy: dscpSelect.value,
      chunk_size: parseInt(chunkSizeSelect.value),
      silence_threshold: silenceThreshold,
      silence_timeout: silenceTimeoutSeconds,
    };

    // Get existing profiles
    const profiles = (await store.get("profiles")) || {};
    profiles[currentProfile] = settings;

    await store.set("profiles", profiles);
    await store.set("current_profile", currentProfile);
    await store.set("device", deviceSelect.value); // Keep device separate for global preference? Or per profile? Let's keep it per profile but maybe fallback.
    // Actually, let's just save everything to the profile.

    await store.save();
    await store.save();
    console.log(`💾 Settings saved to profile '${currentProfile}'`);
    showNotification(
      `Settings saved to profile '${currentProfile}'`,
      "success"
    );
  } catch (e) {
    console.error("❌ Failed to save settings:", e);
    showNotification("Failed to save settings", "error");
  }
}

// Profile Management
async function loadProfiles() {
  const profiles = await store.get("profiles");
  if (!profiles || Object.keys(profiles).length === 0) {
    // Initialize default profile
    await store.set("profiles", { Default: {} });
    await store.set("current_profile", "Default");
    await store.set(
      "silence_threshold",
      parseFloat(silenceThresholdInput.value)
    );
    await store.set("silence_timeout", parseInt(silenceTimeoutInput.value));
    if (loopbackModeInput) {
      await store.set("loopback_mode", loopbackModeInput.checked);
    }
    await store.save();
  }
  await renderProfileList();
}

async function renderProfileList() {
  const profiles = (await store.get("profiles")) || {};
  const current = await store.get("current_profile");

  profileSelect.innerHTML = "";
  Object.keys(profiles).forEach((name) => {
    const option = document.createElement("option");
    option.value = name;
    option.text = name;
    if (name === current) option.selected = true;
    profileSelect.add(option);
  });
}

async function createNewProfile(name) {
  if (!name) return;
  const profiles = (await store.get("profiles")) || {};
  if (profiles[name]) {
    alert("Profile already exists!");
    return;
  }

  // Copy current settings
  const currentProfile = profileSelect.value;
  profiles[name] = profiles[currentProfile] || {};

  await store.set("profiles", profiles);
  await store.set("current_profile", name);
  await store.save();

  await renderProfileList();
  profileSelect.value = name;
  newProfileContainer.style.display = "none";
  showNotification(`Profile '${name}' created successfully`, "success");
}

async function deleteProfile() {
  console.log("🗑️ Delete profile button clicked");
  console.log("Current profile:", profileSelect.value);

  const current = profileSelect.value;
  if (!current) {
    console.warn("No profile selected");
    alert("Please select a profile first");
    return;
  }

  if (current === "Default") {
    console.log("Cannot delete Default profile");
    alert("Cannot delete Default profile");
    return;
  }

  console.log("Deleting profile:", current);
  const profiles = await store.get("profiles");
  console.log("Profiles before delete:", profiles);
  delete profiles[current];
  console.log("Profiles after delete:", profiles);

  await store.set("profiles", profiles);
  await store.set("current_profile", "Default");
  await store.save();

  console.log("✅ Profile deleted, reloading UI");
  await renderProfileList();
  // Reload settings for Default
  await loadSettings();
  showNotification(`Profile '${current}' deleted successfully`, "success");
}

async function toggleStream() {
  if (isStreaming) {
    try {
      await invoke("stop_stream");
      isStreaming = false;
      updateStatus(false, "Ready");
      deviceSelect.disabled = false;
      ipInput.disabled = false;
      portInput.disabled = false;
      sampleRateSelect.disabled = false;
      sampleRateSelect.disabled = false;
      bufferSizeSelect.disabled = false;
      ringBufferDurationSelect.disabled = false;
      // gainSlider remains enabled for real-time adjustment
    } catch (error) {
      updateStatus(true, "Error stopping: " + error);
    }
  } else {
    const savedSilenceTimeout = await store.get("silence_timeout");
    const savedLoopbackMode = await store.get("loopback_mode");

    // Note: The following lines seem to be part of a settings loading logic,
    // but are placed within the toggleStream function's 'start stream' block.
    // This might be a partial or misplaced snippet.
    // Assuming 'savedIp', 'savedPort', etc. are defined elsewhere or intended to be.
    // For now, commenting out the lines that reference undefined 'saved' variables
    // to maintain syntactical correctness based on the provided context.
    // if (savedIp) ipInput.value = savedIp;
    // if (savedPort) portInput.value = savedPort;
    // if (savedSampleRate) sampleRateSelect.value = savedSampleRate;
    // if (savedBufferSize) bufferSizeSelect.value = savedBufferSize;
    // if (savedRingBuffer) ringBufferDurationSelect.value = savedRingBuffer;
    // if (savedAutoReconnect !== null)
    //   autoReconnectCheck.checked = savedAutoReconnect;
    // if (savedHighPriority !== null) priorityCheck.checked = savedHighPriority;
    // if (savedDscp) dscpSelect.value = savedDscp;
    // if (savedChunkSize) chunkSizeSelect.value = savedChunkSize;
    // if (savedSilenceThreshold) silenceThresholdInput.value = savedSilenceThreshold;
    // if (savedSilenceTimeout) silenceTimeoutInput.value = savedSilenceTimeout;

    if (savedLoopbackMode !== null && loopbackModeInput) {
      loopbackMode = savedLoopbackMode;
      loopbackModeInput.checked = savedLoopbackMode;
    }
    const device = deviceSelect.value;
    const ip = ipInput.value;
    const port = parseInt(portInput.value);
    const sampleRate = parseInt(sampleRateSelect.value);
    const bufferSize = parseInt(bufferSizeSelect.value);
    const ringBufferDuration = parseInt(ringBufferDurationSelect.value);
    const autoReconnect = autoReconnectCheck.checked;
    const highPriority = priorityCheck.checked;
    const dscpStrategy = dscpSelect.value;
    const chunkSize = parseInt(chunkSizeSelect.value);

    if (!device) {
      updateStatus(false, "Select a device");
      return;
    }
    if (!ip) {
      updateStatus(false, "Enter Target IP");
      return;
    }
    if (!port) {
      updateStatus(false, "Enter Target Port");
      return;
    }

    const isLoopback = loopbackMode && device.startsWith("[Loopback]");

    try {
      await saveSettings();
      await invoke("start_stream", {
        deviceName: device,
        ip,
        port,
        sampleRate,
        bufferSize,
        ringBufferDurationMs: ringBufferDuration,
        autoReconnect: autoReconnect,
        highPriority: highPriority,
        dscpStrategy: dscpStrategy,
        chunkSize: chunkSize,
        silenceThreshold, // Shorthand for silenceThreshold: silenceThreshold
        silenceTimeoutSeconds: silenceTimeoutSeconds,
        isLoopback: !!isLoopback, // Force boolean to prevent undefined
      });
      isStreaming = true;
      updateStatus(true, "Streaming to " + ip);
      deviceSelect.disabled = true;
      ipInput.disabled = true;
      portInput.disabled = true;
      sampleRateSelect.disabled = true;
      sampleRateSelect.disabled = true;
      bufferSizeSelect.disabled = true;
      ringBufferDurationSelect.disabled = true;
      // gainSlider remains enabled for real-time adjustment
    } catch (error) {
      updateStatus(false, "Error: " + error);
    }
  }
}

async function toggleAutostart() {
  try {
    if (autostartCheck.checked) {
      await enable();
    } else {
      await disable();
    }
  } catch (error) {
    console.error("Autostart error:", error);
  }
}

async function init() {
  console.log("Initializing app...");

  // Initialize Store first (Tauri Plugin Store v2 API)
  try {
    store = await Store.load("store.bin");
    console.log("✅ Store initialized successfully");
    console.log("Store instance:", store);
    console.log(
      "Store methods:",
      Object.getOwnPropertyNames(Object.getPrototypeOf(store))
    );
  } catch (e) {
    console.error("❌ Failed to initialize store:", e);
    // Create a fallback mock store to prevent crashes
    store = {
      get: async () => null,
      set: async () => {},
      save: async () => {},
      delete: async () => {},
    };
  }

  // Initialize elements
  deviceSelect = document.getElementById("device-select");
  ipInput = document.getElementById("ip-input");
  portInput = document.getElementById("port-input");
  sampleRateSelect = document.getElementById("sample-rate");
  bufferSizeSelect = document.getElementById("buffer-size");
  ringBufferDurationSelect = document.getElementById("ring-buffer-duration");
  autostartCheck = document.getElementById("autostart-check");
  autostreamCheck = document.getElementById("autostream-check");
  autoReconnectCheck = document.getElementById("autoreconnect-check");
  priorityCheck = document.getElementById("priority-check");
  dscpSelect = document.getElementById("dscp-select");
  chunkSizeSelect = document.getElementById("chunk-size-select");
  const silenceThresholdInput = document.getElementById("silence-threshold");
  const silenceTimeoutInput = document.getElementById("silence-timeout");
  toggleBtn = document.getElementById("toggle-btn");
  statusBadge = document.getElementById("status-badge");
  statusText = document.getElementById("status-text");
  loopbackModeInput = document.getElementById("loopback-mode");

  // Tabs
  tabBtns = document.querySelectorAll(".tab-btn");
  tabPanes = document.querySelectorAll(".tab-pane");

  // Profile Elements
  profileSelect = document.getElementById("profile-select");
  btnSaveProfile = document.getElementById("btn-save-profile");
  btnNewProfile = document.getElementById("btn-new-profile");
  btnDeleteProfile = document.getElementById("btn-delete-profile");
  newProfileContainer = document.getElementById("new-profile-container");
  newProfileName = document.getElementById("new-profile-name");
  btnConfirmProfile = document.getElementById("btn-confirm-profile");
  btnCancelProfile = document.getElementById("btn-cancel-profile");

  // Initialize new elements
  logsContainer = document.getElementById("logs-container");
  clearLogsBtn = document.getElementById("clear-logs-btn");
  const logFilter = document.getElementById("log-filter");
  statsBar = document.getElementById("stats-bar");

  if (logFilter) {
    logFilter.addEventListener("change", (e) => {
      filterLogs(e.target.value);
    });
  }

  // Set up Tauri event listeners
  await listen("log-event", (event) => {
    addLog(event.payload);
  });

  await listen("stats-event", (event) => {
    updateStats(event.payload);
  });

  await listen("health-event", (event) => {
    const { buffer_usage, dropped_packets } = event.payload;
    // Only log if something is wrong or periodically
    if (buffer_usage > 0.8) {
      addLog({
        timestamp: new Date().toLocaleTimeString(),
        level: "warning",
        message: `High Buffer Usage: ${(buffer_usage * 100).toFixed(
          1
        )}% (Network Slow)`,
      });
    }
    if (dropped_packets > 0) {
      // We could update a UI element here instead of spamming logs
      // For now, let's just log it once if it changes?
      // Actually, let's just update the stats bar with health info
      const statsText = document.getElementById("stats-text");
      if (statsText) {
        // Append health info to existing stats
        // This is a bit hacky, but works for now
      }
    }
  });

  // Gain slider removed

  // Auto-save on input changes
  if (deviceSelect) deviceSelect.addEventListener("change", saveSettings);
  if (ipInput) ipInput.addEventListener("change", saveSettings);
  if (portInput) portInput.addEventListener("change", saveSettings);
  if (streamNameInput) streamNameInput.addEventListener("change", saveSettings);
  if (sampleRateSelect)
    sampleRateSelect.addEventListener("change", saveSettings);
  if (bufferSizeSelect)
    bufferSizeSelect.addEventListener("change", saveSettings);
  if (ringBufferDurationSelect)
    ringBufferDurationSelect.addEventListener("change", saveSettings);
  if (autostreamCheck) autostreamCheck.addEventListener("change", saveSettings);
  if (autoReconnectCheck)
    autoReconnectCheck.addEventListener("change", saveSettings);
  if (priorityCheck) priorityCheck.addEventListener("change", saveSettings);
  if (dscpSelect) dscpSelect.addEventListener("change", saveSettings);
  if (chunkSizeSelect) chunkSizeSelect.addEventListener("change", saveSettings);
  if (silenceThresholdInput)
    silenceThresholdInput.addEventListener("change", saveSettings);
  if (silenceTimeoutInput)
    silenceTimeoutInput.addEventListener("change", saveSettings);

  if (loopbackModeInput) {
    loopbackModeInput.addEventListener("change", async (e) => {
      loopbackMode = e.target.checked;
      saveSettings();
      await loadDevices();
    });
  }

  // Logs toggle (Removed)

  // Clear logs button
  if (clearLogsBtn) {
    clearLogsBtn.addEventListener("click", clearLogs);
  }

  // Advanced settings toggle (Removed)

  if (toggleBtn) {
    toggleBtn.addEventListener("click", toggleStream);
  }

  if (autostartCheck) {
    autostartCheck.addEventListener("change", toggleAutostart);
  }

  // Tab Listeners
  tabBtns.forEach((btn) => {
    btn.addEventListener("click", () => {
      // Remove active class from all
      tabBtns.forEach((b) => b.classList.remove("active"));
      tabPanes.forEach((p) => p.classList.remove("active"));

      // Add active class to clicked
      btn.classList.add("active");
      const tabId = btn.getAttribute("data-tab");
      document.getElementById(tabId).classList.add("active");
    });
  });

  // Profile Listeners
  if (profileSelect) {
    profileSelect.addEventListener("change", async (e) => {
      await store.set("current_profile", e.target.value);
      await store.save();
      await loadSettings();
    });
  }

  if (btnSaveProfile) btnSaveProfile.addEventListener("click", saveSettings);

  if (btnNewProfile) {
    btnNewProfile.addEventListener("click", () => {
      newProfileContainer.style.display = "flex";
      newProfileName.focus();
    });
  }

  console.log("Profile buttons check:", {
    btnSaveProfile: !!btnSaveProfile,
    btnNewProfile: !!btnNewProfile,
    btnDeleteProfile: !!btnDeleteProfile,
  });

  if (btnDeleteProfile) {
    console.log("✅ Delete button found, attaching event listener");
    console.log("Delete button element:", btnDeleteProfile);
    console.log("Delete function exists:", typeof deleteProfile);

    // Try both ways
    btnDeleteProfile.addEventListener("click", deleteProfile);
    btnDeleteProfile.onclick = () => {
      console.log("🔥 ONCLICK HANDLER FIRED (backup)");
      deleteProfile();
    };
  } else {
    console.error("❌ Delete button NOT found!");
  }

  if (btnConfirmProfile) {
    btnConfirmProfile.addEventListener("click", () => {
      createNewProfile(newProfileName.value);
    });
  }

  if (btnCancelProfile) {
    btnCancelProfile.addEventListener("click", () => {
      newProfileContainer.style.display = "none";
      newProfileName.value = "";
    });
  }

  // EQ listeners removed

  // Load data
  await loadSettings();
  await loadDevices();

  // Auto-stream check
  if (autostreamCheck && autostreamCheck.checked) {
    // Small delay to ensure devices are loaded
    setTimeout(() => {
      if (deviceSelect.value && ipInput.value) {
        toggleStream();
      }
    }, 500);
  }

  console.log("App initialized");
}

window.addEventListener("DOMContentLoaded", init);
