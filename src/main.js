console.log("Main.js loaded");
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";
import { Store } from "@tauri-apps/plugin-store";


let store; // Store will be initialized async

// Debug Store (will log after init)

let isStreaming = false;
let deviceSelect, ipInput, portInput, sampleRateSelect, bufferSizeSelect, codecSelect, autostartCheck, autostreamCheck, autoReconnectCheck, toggleBtn, statusBadge, statusText;
let adaptiveBitrateCheck, statLatency, suggestedAction, actionValue;
let profileSelect, btnSaveProfile, btnNewProfile, btnDeleteProfile, newProfileContainer, newProfileName, btnConfirmProfile, btnCancelProfile;
let logsContainer, clearLogsBtn, statsBar;
let tabBtns, tabPanes;

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
function addLog(log) {
    const entry = document.createElement('div');
    entry.className = `log-entry log-${log.level}`;
    entry.innerHTML = `<span class="log-time">[${log.timestamp}]</span> ${log.message}`;
    
    logsContainer.appendChild(entry);
    
    // Limit logs to MAX_LOGS
    while (logsContainer.children.length > MAX_LOGS) {
        logsContainer.removeChild(logsContainer.firstChild);
    }
    
    // Auto-scroll to bottom
    logsContainer.scrollTop = logsContainer.scrollHeight;
}

function clearLogs() {
    logsContainer.innerHTML = '';
}

// Statistics formatting
function formatUptime(seconds) {
    const hrs = Math.floor(seconds / 3600);
    const mins = Math.floor((seconds % 3600) / 60);
    const secs = seconds % 60;
    
    if (hrs > 0) {
        return `${hrs}:${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
    }
    return `${mins}:${secs.toString().padStart(2, '0')}`;
}

function formatBytes(bytes) {
    if (bytes === 0) return '0 MB';
    const mb = bytes / (1024 * 1024);
    if (mb < 1) {
        return (bytes / 1024).toFixed(1) + ' KB';
    }
    return mb.toFixed(2) + ' MB';
}

function updateStats(stats) {
    document.getElementById('stat-uptime').textContent = formatUptime(stats.uptime_seconds);
    document.getElementById('stat-bytes').textContent = formatBytes(stats.bytes_sent);
    document.getElementById('stat-bitrate').textContent = stats.bitrate_kbps.toFixed(1) + ' kbps';
    if (statLatency) statLatency.textContent = stats.buffer_latency_ms.toFixed(1) + ' ms';
}

function updateAdaptiveVisibility() {
    const container = document.getElementById("adaptive-container");
    if (container && codecSelect) {
        if (codecSelect.value === "2") { // Opus
            container.style.display = "block";
        } else {
            container.style.display = "none";
        }
    }
}

async function loadDevices() {
  try {
    const devices = await invoke("get_input_devices");
    deviceSelect.innerHTML = "";
    
    if (devices.length === 0) {
        const option = document.createElement("option");
        option.text = "No devices found";
        option.disabled = true;
        deviceSelect.add(option);
        return;
    }

    devices.forEach((device) => {
      const name = device.name || device; // Handle object or string
      const option = document.createElement("option");
      option.value = name;
      option.text = name;
      deviceSelect.add(option);
    });
    
    // Restore selected device if exists
    const savedDevice = await store.get("device");
    // Or from profile
    const currentProfile = await store.get("current_profile");
    const profiles = await store.get("profiles") || {};
    const profileData = profiles[currentProfile] || {};
    
    const deviceToSelect = profileData.device || savedDevice;
    
    if (deviceToSelect) {
        // Check if device still exists
        if (devices.includes(deviceToSelect)) {
            deviceSelect.value = deviceToSelect;
        }
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
        
        const currentProfile = await store.get("current_profile") || "Default";
        const profiles = await store.get("profiles") || {};
        const settings = profiles[currentProfile] || {};
        
        console.log(`Loading profile: ${currentProfile}`, settings);

        if (settings.ip) ipInput.value = settings.ip;
        if (settings.port) portInput.value = settings.port;
        if (settings.sample_rate) sampleRateSelect.value = settings.sample_rate;
        if (settings.buffer_size) bufferSizeSelect.value = settings.buffer_size;
        if (settings.codec) codecSelect.value = settings.codec; // Added codec
        if (settings.adaptive_bitrate !== undefined) adaptiveBitrateCheck.checked = settings.adaptive_bitrate;
        if (settings.auto_stream !== undefined) autostreamCheck.checked = settings.auto_stream;
        if (settings.auto_reconnect !== undefined) autoReconnectCheck.checked = settings.auto_reconnect;
        
        updateAdaptiveVisibility();
        
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
            buffer_size: parseInt(bufferSizeSelect.value),
            codec: parseInt(codecSelect.value), // Added codec
            adaptive_bitrate: adaptiveBitrateCheck.checked,
            auto_stream: autostreamCheck.checked,
            auto_reconnect: autoReconnectCheck.checked
        };

        // Get existing profiles
        const profiles = await store.get("profiles") || {};
        profiles[currentProfile] = settings;
        
        await store.set("profiles", profiles);
        await store.set("current_profile", currentProfile);
        await store.set("device", deviceSelect.value); // Keep device separate for global preference? Or per profile? Let's keep it per profile but maybe fallback.
        // Actually, let's just save everything to the profile.
        
        await store.save();
        console.log(`💾 Settings saved to profile '${currentProfile}'`);
    } catch (e) {
        console.error("❌ Failed to save settings:", e);
    }
}

// Profile Management
async function loadProfiles() {
    const profiles = await store.get("profiles");
    if (!profiles || Object.keys(profiles).length === 0) {
        // Initialize default profile
        await store.set("profiles", { "Default": {} });
        await store.set("current_profile", "Default");
        await store.save();
    }
    await renderProfileList();
}

async function renderProfileList() {
    const profiles = await store.get("profiles") || {};
    const current = await store.get("current_profile");
    
    profileSelect.innerHTML = "";
    Object.keys(profiles).forEach(name => {
        const option = document.createElement("option");
        option.value = name;
        option.text = name;
        if (name === current) option.selected = true;
        profileSelect.add(option);
    });
}

async function createNewProfile(name) {
    if (!name) return;
    
    const profiles = await store.get("profiles") || {};
    if (profiles[name]) {
        alert("Profile already exists!");
        return;
    }
    
    // Clone current settings
    const currentSettings = {
        device: deviceSelect.value,
        ip: ipInput.value,
        port: parseInt(portInput.value),
        sample_rate: parseInt(sampleRateSelect.value),
        buffer_size: parseInt(bufferSizeSelect.value),
        codec: parseInt(codecSelect.value),
        adaptive_bitrate: adaptiveBitrateCheck.checked,
        auto_stream: autostreamCheck.checked,
        auto_reconnect: autoReconnectCheck.checked
    };
    
    profiles[name] = currentSettings;
    await store.set("profiles", profiles);
    await store.set("current_profile", name);
    await store.save();
    
    newProfileContainer.style.display = "none";
    newProfileName.value = "";
    
    await renderProfileList();
    // Select new profile
    profileSelect.value = name;
    console.log(`✅ Created profile: ${name}`);
}

async function deleteProfile() {
    const current = profileSelect.value;
    if (current === "Default") {
        alert("Cannot delete Default profile");
        return;
    }
    
    if (!confirm(`Delete profile '${current}'?`)) return;
    
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
      bufferSizeSelect.disabled = false;
      codecSelect.disabled = false;
      adaptiveBitrateCheck.disabled = false;
      // gainSlider remains enabled for real-time adjustment
    } catch (error) {
      updateStatus(true, "Error stopping: " + error);
    }
  } else {
    const device = deviceSelect.value;
    const ip = ipInput.value;
    const port = parseInt(portInput.value);
    const sampleRate = parseInt(sampleRateSelect.value);
    const bufferSize = parseInt(bufferSizeSelect.value);
    const codec = parseInt(codecSelect.value);
    const adaptiveBitrate = adaptiveBitrateCheck.checked;
    const autoReconnect = autoReconnectCheck.checked;

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

    try {
      await saveSettings();
      await invoke("start_stream", { 
          deviceName: device, 
          ip, 
          port,
          sampleRate,
          bufferSize,
          codec,
          adaptiveBitrate,
          autoReconnect: autoReconnect,
          appHandle: null // Backend handles this
      });
      isStreaming = true;
      updateStatus(true, "Streaming to " + ip);
      deviceSelect.disabled = true;
      ipInput.disabled = true;
      portInput.disabled = true;
      sampleRateSelect.disabled = true;
      bufferSizeSelect.disabled = true;
      codecSelect.disabled = true;
      adaptiveBitrateCheck.disabled = true;
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

  // Initialize Tabs immediately to ensure navigation works
  tabBtns = document.querySelectorAll('.tab-btn');
  tabPanes = document.querySelectorAll('.tab-pane');

  tabBtns.forEach(btn => {
      btn.addEventListener('click', () => {
          console.log("Tab clicked:", btn.getAttribute('data-tab'));
          // Remove active class from all
          tabBtns.forEach(b => b.classList.remove('active'));
          tabPanes.forEach(p => p.classList.remove('active'));
          
          // Add active class to clicked
          btn.classList.add('active');
          const tabId = btn.getAttribute('data-tab');
          const tabPane = document.getElementById(tabId);
          if (tabPane) {
              tabPane.classList.add('active');
          } else {
              console.error(`Tab pane not found for id: ${tabId}`);
          }
      });
  });

  // Initialize Store first (Tauri Plugin Store v2 API)
  try {
    store = await Store.load("store.bin");
    console.log("✅ Store initialized successfully");
    console.log("Store instance:", store);
    console.log("Store methods:", Object.getOwnPropertyNames(Object.getPrototypeOf(store)));
  } catch (e) {
    console.error("❌ Failed to initialize store:", e);
    // Create a fallback mock store to prevent crashes
    store = {
      get: async () => null,
      set: async () => {},
      save: async () => {},
      delete: async () => {}
    };
  }
  
  // Initialize elements
  deviceSelect = document.getElementById("device-select");
  ipInput = document.getElementById("ip-input");
  portInput = document.getElementById("port-input");
  sampleRateSelect = document.getElementById("sample-rate");
  bufferSizeSelect = document.getElementById("buffer-size");
  codecSelect = document.getElementById("codec-select"); // Added codecSelect initialization
  adaptiveBitrateCheck = document.getElementById("adaptive-bitrate-check");
  autostartCheck = document.getElementById("autostart-check");
  autostreamCheck = document.getElementById("autostream-check");
  autoReconnectCheck = document.getElementById("autoreconnect-check");
  toggleBtn = document.getElementById("toggle-btn");
  statusBadge = document.getElementById("status-badge");
  statusText = document.getElementById("status-text");
  
  
  // Tabs (Initialized at top)


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
  statsBar = document.getElementById("stats-bar");
  statLatency = document.getElementById("stat-latency");
  suggestedAction = document.getElementById("suggested-action");
  actionValue = document.getElementById("action-value");

  // Set up Tauri event listeners
  await listen('log-event', (event) => {
    addLog(event.payload);
  });

  await listen('stats-event', (event) => {
    updateStats(event.payload);
  });
  
  await listen('health-event', (event) => {
      if (suggestedAction && actionValue) {
          suggestedAction.style.display = "block";
          actionValue.textContent = event.payload.suggested_action;
          
          // Color code
          const status = event.payload.status;
          if (status === "Poor") actionValue.style.color = "#ff4444";
          else if (status === "Degraded") actionValue.style.color = "#ffbb33";
          else actionValue.style.color = "#00C851";
      }
      
      if (document.getElementById("health-value")) {
          const healthBadge = document.getElementById("health-badge");
          const healthValue = document.getElementById("health-value");
          healthBadge.style.display = "block";
          healthValue.textContent = event.payload.status;
      }
  });

  // Gain slider removed

  // Auto-save on input changes
  if (deviceSelect) deviceSelect.addEventListener('change', saveSettings);
  if (ipInput) ipInput.addEventListener('change', saveSettings);
  if (portInput) portInput.addEventListener('change', saveSettings);
  if (sampleRateSelect) sampleRateSelect.addEventListener('change', saveSettings);
  if (bufferSizeSelect) bufferSizeSelect.addEventListener('change', saveSettings);
  if (codecSelect) {
      codecSelect.addEventListener('change', () => {
          saveSettings();
          updateAdaptiveVisibility();
      });
  }
  if (adaptiveBitrateCheck) adaptiveBitrateCheck.addEventListener('change', saveSettings);
  if (autostreamCheck) autostreamCheck.addEventListener('change', saveSettings);
  if (autoReconnectCheck) autoReconnectCheck.addEventListener('change', saveSettings);

  // Logs toggle (Removed)
  
  // Clear logs button
  if (clearLogsBtn) {
    clearLogsBtn.addEventListener('click', clearLogs);
  }

  // Advanced settings toggle (Removed)
  
  if (toggleBtn) {
      toggleBtn.addEventListener("click", toggleStream);
  }
  
  if (autostartCheck) {
      autostartCheck.addEventListener("change", toggleAutostart);
  }

    // Tab Listeners (Initialized at top)


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
        btnDeleteProfile: !!btnDeleteProfile
    });
    
    
    if (btnDeleteProfile) {
        console.log("✅ Delete button found, attaching event listener");
        console.log("Delete button element:", btnDeleteProfile);
        console.log("Delete function exists:", typeof deleteProfile);
        
        // Try both ways
        btnDeleteProfile.addEventListener("click", deleteProfile);
        btnDeleteProfile.onclick =() => {
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

if (document.readyState === "loading") {
    window.addEventListener("DOMContentLoaded", init);
} else {
    init();
}
