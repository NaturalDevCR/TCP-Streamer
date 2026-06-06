<template>
  <div class="animate-fade-in space-y-4">
    <!-- Source -->
    <section class="glass-card">
      <h3 class="text-sm font-semibold text-slate-300 border-b border-white/10 pb-2 mb-4">
        Source
      </h3>
      <SelectField
        id="device-select"
        v-model="settings.deviceName"
        label="Input Device"
        :disabled="stream.isStreaming || settings.devicesLoading"
      >
        <option v-if="settings.devicesLoading" disabled>Loading devices...</option>
        <option v-else-if="settings.devices.length === 0" disabled>No devices found</option>
        <option v-for="d in settings.devices" :key="d" :value="d">{{ d }}</option>
      </SelectField>

      <p v-if="settings.devicesError" class="text-xs text-red-400 mt-1 ml-0.5">
        {{ settings.devicesError }}
      </p>

      <!-- Loopback (Windows only) -->
      <div v-if="settings.showLoopback" class="mt-3">
        <CheckboxField
          v-model="settings.loopbackMode"
          label="Enable Loopback (Windows)"
          description="Capture system audio directly without virtual cables (requires speakers/headphones connected)"
          @update:model-value="onLoopbackChange"
        />
      </div>
    </section>

    <!-- Connection Mode -->
    <section class="glass-card">
      <h3 class="text-sm font-semibold text-slate-300 border-b border-white/10 pb-2 mb-4">
        Connection
      </h3>
      <SelectField id="mode-select" v-model="settings.mode" :disabled="stream.isStreaming">
        <option value="client">Client (Send Audio to IP)</option>
        <option value="server">Server (Listen for Connections)</option>
      </SelectField>
      <p class="text-[11px] text-white/50 mt-2 leading-relaxed">
        Client Mode: Connects to a Snapserver.<br />
        Server Mode: Waits for Snapservers to connect.
      </p>
    </section>

    <!-- Destination / Server Settings -->
    <section class="glass-card">
      <h3 class="text-sm font-semibold text-slate-300 border-b border-white/10 pb-2 mb-4">
        {{ settings.isServer ? "TCP Server Settings" : "TCP Destination" }}
      </h3>
      <div class="flex gap-3">
        <InputField
          v-if="!settings.isServer"
          id="ip-input"
          v-model="settings.ip"
          label="Target IP"
          placeholder="192.168.1.100"
          :disabled="stream.isStreaming"
        />
        <InputField
          id="port-input"
          v-model.number="settings.port"
          :label="settings.isServer ? 'Listen Port' : 'Port'"
          type="number"
          placeholder="1704"
          :disabled="stream.isStreaming"
        />
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { useSettingsStore } from "../../stores/settings.ts";
import { useStreamStore } from "../../stores/stream.ts";
import SelectField from "../ui/SelectField.vue";
import InputField from "../ui/InputField.vue";
import CheckboxField from "../ui/CheckboxField.vue";

const settings = useSettingsStore();
const stream = useStreamStore();

async function onLoopbackChange() {
  await settings.loadDevices();
}
</script>
