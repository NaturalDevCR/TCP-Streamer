import { invoke } from "@tauri-apps/api/core";
import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";
import { Store } from "@tauri-apps/plugin-store";

const store = new Store("store.bin");

let isStreaming = false;
let deviceSelect, ipInput, portInput, sampleRateSelect, bufferSizeSelect, autostartCheck, autostreamCheck, toggleBtn, statusBadge, statusText, advancedToggle, advancedSection;

function updateStatus(active, text) {
    if (active) {
        statusBadge.classList.add("active");
        toggleBtn.classList.add("stop");
        toggleBtn.querySelector(".btn-text").textContent = "Stop Streaming";
    } else {
        statusBadge.classList.remove("active");
        toggleBtn.classList.remove("stop");
        toggleBtn.querySelector(".btn-text").textContent = "Start Streaming";
    }
    statusText.textContent = text;
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
        await store.load();
        
        const savedIp = await store.get("ip");
        if (savedIp) ipInput.value = savedIp;
        
        const savedPort = await store.get("port");
        if (savedPort) portInput.value = savedPort;

        const savedSampleRate = await store.get("sample_rate");
        if (savedSampleRate) sampleRateSelect.value = savedSampleRate;

        const savedBufferSize = await store.get("buffer_size");
        if (savedBufferSize) bufferSizeSelect.value = savedBufferSize;

        const savedAutoStream = await store.get("auto_stream");
        if (savedAutoStream) autostreamCheck.checked = savedAutoStream;
    } catch (e) {
        console.warn("Failed to load settings from store:", e);
    }

    try {
        const autostart = await isEnabled();
        autostartCheck.checked = autostart;
    } catch (e) {
        console.warn("Failed to check autostart status:", e);
    }
}

async function saveSettings() {
    try {
        await store.set("device", deviceSelect.value);
        await store.set("ip", ipInput.value);
        await store.set("port", parseInt(portInput.value));
        await store.set("sample_rate", parseInt(sampleRateSelect.value));
        await store.set("buffer_size", parseInt(bufferSizeSelect.value));
        await store.set("auto_stream", autostreamCheck.checked);
        await store.save();
    } catch (e) {
        console.error("Failed to save settings:", e);
    }
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
    } catch (error) {
      updateStatus(true, "Error stopping: " + error);
    }
  } else {
    const device = deviceSelect.value;
    const ip = ipInput.value;
    const port = parseInt(portInput.value);
    const sampleRate = parseInt(sampleRateSelect.value);
    const bufferSize = parseInt(bufferSizeSelect.value);

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
          bufferSize
      });
      isStreaming = true;
      updateStatus(true, "Streaming to " + ip);
      deviceSelect.disabled = true;
      ipInput.disabled = true;
      portInput.disabled = true;
      sampleRateSelect.disabled = true;
      bufferSizeSelect.disabled = true;
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
  
  // Initialize elements
  deviceSelect = document.getElementById("device-select");
  ipInput = document.getElementById("ip-input");
  portInput = document.getElementById("port-input");
  sampleRateSelect = document.getElementById("sample-rate");
  bufferSizeSelect = document.getElementById("buffer-size");
  autostartCheck = document.getElementById("autostart-check");
  autostreamCheck = document.getElementById("autostream-check");
  toggleBtn = document.getElementById("toggle-btn");
  statusBadge = document.getElementById("status-badge");
  statusText = document.getElementById("status-text");
  advancedToggle = document.getElementById("advanced-toggle");
  advancedSection = document.getElementById("advanced-section");

  // UI Interactions
  if (advancedToggle) {
      advancedToggle.addEventListener("click", () => {
          console.log("Toggling advanced settings");
          advancedToggle.classList.toggle("open");
          advancedSection.classList.toggle("open");
      });
  } else {
      console.error("Advanced toggle element not found");
  }
  
  if (toggleBtn) {
      toggleBtn.addEventListener("click", toggleStream);
  }
  
  if (autostartCheck) {
      autostartCheck.addEventListener("change", toggleAutostart);
  }

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
