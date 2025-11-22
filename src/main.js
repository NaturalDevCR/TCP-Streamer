import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";
import { Store } from "@tauri-apps/plugin-store";


let store; // Store will be initialized async

// Debug Store (will log after init)

let isStreaming = false;
let deviceSelect, ipInput, portInput, sampleRateSelect, bufferSizeSelect, autostartCheck, autostreamCheck, autoReconnectCheck, toggleBtn, statusBadge, statusText;
let profileSelect, btnSaveProfile, btnNewProfile, btnDeleteProfile, newProfileContainer, newProfileName, btnConfirmProfile, btnCancelProfile;
let logsContainer, clearLogsBtn, gainSlider, gainValue, statsBar;
let tabBtns, tabPanes;
let eqToggle, eqControls;
let eqBass, eqMid, eqTreble, eqBassVal, eqMidVal, eqTrebleVal;

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
}

async function loadDevices() {
  try {
    console.log("Requesting devices...");
    const devices = await invoke("get_input_devices");
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
      option.value = device.name;
      option.text = device.name;
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
        
        const currentProfile = await store.get("current_profile") || "Default";
        const profiles = await store.get("profiles") || {};
        const settings = profiles[currentProfile] || {};
        
        console.log(`Loading profile: ${currentProfile}`, settings);

        if (settings.ip) ipInput.value = settings.ip;
        if (settings.port) portInput.value = settings.port;
        if (settings.sample_rate) sampleRateSelect.value = settings.sample_rate;
        if (settings.buffer_size) bufferSizeSelect.value = settings.buffer_size;
        if (settings.auto_stream !== undefined) autostreamCheck.checked = settings.auto_stream;
        if (settings.auto_reconnect !== undefined) autoReconnectCheck.checked = settings.auto_reconnect;
        
        if (settings.gain !== undefined) {
            gainSlider.value = settings.gain;
            gainValue.textContent = Math.round(settings.gain * 100) + '%';
        }

        if (settings.eq_enabled !== undefined) eqToggle.checked = settings.eq_enabled;
        if (settings.eq_bass !== undefined) {
            eqBass.value = settings.eq_bass;
            eqBassVal.textContent = (settings.eq_bass > 0 ? '+' : '') + settings.eq_bass + 'dB';
        }
        if (settings.eq_mid !== undefined) {
            eqMid.value = settings.eq_mid;
            eqMidVal.textContent = (settings.eq_mid > 0 ? '+' : '') + settings.eq_mid + 'dB';
        }
        if (settings.eq_treble !== undefined) {
            eqTreble.value = settings.eq_treble;
            eqTrebleVal.textContent = (settings.eq_treble > 0 ? '+' : '') + settings.eq_treble + 'dB';
        }
        // Visualizer removed
        
        // Update UI state
        eqControls.style.display = eqToggle.checked ? 'flex' : 'none';
        
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
            auto_stream: autostreamCheck.checked,
            auto_reconnect: autoReconnectCheck.checked,
            gain: parseFloat(gainSlider.value),
            eq_enabled: eqToggle.checked,
            eq_bass: parseFloat(eqBass.value),
            eq_mid: parseFloat(eqMid.value),
            eq_treble: parseFloat(eqTreble.value)
        };

        // Get existing profiles
        const profiles = await store.get("profiles") || {};
        profiles[currentProfile] = settings;
        
        await store.set("profiles", profiles);
        await store.set("current_profile", currentProfile);
        await store.set("device", deviceSelect.value); // Keep device separate for global preference? Or per profile? Let's keep it per profile but maybe fallback.
        // Actually, let's just save everything to the profile.
        
        await store.save();
        console.log(`✅ Settings saved to profile '${currentProfile}'`);
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
    
    // Copy current settings
    const currentProfile = profileSelect.value;
    profiles[name] = profiles[currentProfile] || {};
    
    await store.set("profiles", profiles);
    await store.set("current_profile", name);
    await store.save();
    
    await renderProfileList();
    profileSelect.value = name;
    newProfileContainer.style.display = "none";
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
    const gain = parseFloat(gainSlider.value); // 0.0 - 2.0

    const eqSettings = {
        enabled: eqToggle.checked,
        bass_gain: parseFloat(eqBass.value),
        mid_gain: parseFloat(eqMid.value),
        treble_gain: parseFloat(eqTreble.value)
    };
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
          gain,
          eqSettings: eqSettings,
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
  autostartCheck = document.getElementById("autostart-check");
  autostreamCheck = document.getElementById("autostream-check");
  autoReconnectCheck = document.getElementById("autoreconnect-check");
  toggleBtn = document.getElementById("toggle-btn");
  statusBadge = document.getElementById("status-badge");
  statusText = document.getElementById("status-text");
  
  // Tabs
  tabBtns = document.querySelectorAll('.tab-btn');
  tabPanes = document.querySelectorAll('.tab-pane');

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
  gainSlider = document.getElementById("gain-slider");
  gainValue = document.getElementById("gain-value");
  statsBar = document.getElementById("stats-bar");

    // EQ Elements
    eqToggle = document.getElementById('eq-toggle');
    eqControls = document.getElementById('eq-controls');
    eqBass = document.getElementById('eq-bass');
    eqMid = document.getElementById('eq-mid');
    eqTreble = document.getElementById('eq-treble');
    eqBassVal = document.getElementById('eq-bass-val');
    eqMidVal = document.getElementById('eq-mid-val');
    eqTrebleVal = document.getElementById('eq-treble-val');

  // Set up Tauri event listeners
  await listen('log-event', (event) => {
    addLog(event.payload);
  });

  await listen('stats-event', (event) => {
    updateStats(event.payload);
  });

  // Gain slider
  if (gainSlider && gainValue) {
    gainSlider.addEventListener('input', (e) => {
      gainValue.textContent = Math.round(e.target.value * 100) + '%';
    });
    gainSlider.addEventListener('change', saveSettings); // Auto-save on change
  }

  // Auto-save on input changes
  if (deviceSelect) deviceSelect.addEventListener('change', saveSettings);
  if (ipInput) ipInput.addEventListener('change', saveSettings);
  if (portInput) portInput.addEventListener('change', saveSettings);
  if (sampleRateSelect) sampleRateSelect.addEventListener('change', saveSettings);
  if (bufferSizeSelect) bufferSizeSelect.addEventListener('change', saveSettings);
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

    // Tab Listeners
    tabBtns.forEach(btn => {
        btn.addEventListener('click', () => {
            // Remove active class from all
            tabBtns.forEach(b => b.classList.remove('active'));
            tabPanes.forEach(p => p.classList.remove('active'));
            
            // Add active class to clicked
            btn.classList.add('active');
            const tabId = btn.getAttribute('data-tab');
            document.getElementById(tabId).classList.add('active');
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

    // Initialize listeners
    eqToggle.addEventListener('change', () => {
        eqControls.style.display = eqToggle.checked ? 'flex' : 'none';
        saveSettings();
    });

    [eqBass, eqMid, eqTreble].forEach(slider => {
        slider.addEventListener('input', (e) => {
            const valSpan = document.getElementById(e.target.id + '-val');
            valSpan.textContent = (e.target.value > 0 ? '+' : '') + e.target.value + 'dB';
        });
        slider.addEventListener('change', saveSettings);
    });
    
    // Initial EQ state
    eqControls.style.display = eqToggle.checked ? 'flex' : 'none';

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
