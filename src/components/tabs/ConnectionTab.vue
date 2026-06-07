<template>
  <div class="animate-fade-in space-y-4">
    <!-- Role -->
    <section class="glass-card">
      <h3 class="text-sm font-semibold text-slate-300 border-b border-white/10 pb-2 mb-4">Role</h3>
      <SelectField
        id="role-select"
        v-model="settings.role"
        label="Role"
        :disabled="stream.isStreaming"
      >
        <option value="source">Source (capture &amp; send)</option>
        <option value="sink">Sink (receive &amp; play)</option>
      </SelectField>
    </section>

    <!-- Sink -->
    <section v-if="settings.role === 'sink'" class="glass-card">
      <h3 class="text-sm font-semibold text-slate-300 border-b border-white/10 pb-2 mb-4">
        Sink Settings
      </h3>
      <SelectField
        id="output-device"
        v-model="settings.outputDevice"
        label="Output Device"
        :disabled="stream.isStreaming"
      >
        <option v-if="settings.outputDevices.length === 0" disabled>No output devices found</option>
        <option v-for="d in settings.outputDevices" :key="d" :value="d">{{ d }}</option>
      </SelectField>
      <InputField
        id="source-addr"
        v-model="settings.sourceAddr"
        label="Source Address"
        placeholder="192.168.1.50:4953"
        :disabled="stream.isStreaming"
        class="mt-3"
      />

      <!-- mDNS Scan -->
      <div class="mt-3 flex gap-2">
        <button
          class="px-3 py-1.5 text-xs bg-white/10 hover:bg-white/15 text-slate-200 rounded-lg border border-white/10 transition-colors"
          :disabled="stream.isStreaming"
          @click="settings.scanSources()"
        >
          Scan LAN
        </button>
        <span v-if="settings.discoveredSources.length" class="text-xs text-white/40 self-center">
          {{ settings.discoveredSources.length }} found
        </span>
      </div>
      <SelectField
        v-if="settings.discoveredSources.length"
        id="discovered-sources"
        :model-value="settings.sourceAddr"
        label="Discovered Sources"
        :disabled="stream.isStreaming"
        class="mt-2"
        @update:model-value="(v: string) => (settings.sourceAddr = v)"
      >
        <option value="" disabled>Choose a source...</option>
        <option v-for="s in settings.discoveredSources" :key="s.addr" :value="s.addr">
          {{ s.name }} ({{ s.addr }}){{ s.encrypted ? " [encrypted]" : "" }}
        </option>
      </SelectField>

      <InputField
        id="psk-sink"
        v-model="settings.psk"
        label="Encryption key (PSK — empty = no encryption)"
        type="password"
        placeholder="Pre-shared key"
        :disabled="stream.isStreaming"
        class="mt-3"
      />
    </section>

    <!-- Source -->
    <section v-if="settings.role === 'source'" class="glass-card">
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

      <!-- Transport (Source only) -->
      <SelectField
        id="transport-select"
        v-model="settings.transport"
        label="Transport"
        :disabled="stream.isStreaming"
        class="mt-3"
      >
        <option value="tcp">TCP</option>
        <option value="udp">Native UDP</option>
      </SelectField>

      <!-- PSK (only for Native UDP) -->
      <InputField
        v-if="settings.transport === 'udp'"
        id="psk-source"
        v-model="settings.psk"
        label="Encryption key (PSK — empty = no encryption)"
        type="password"
        placeholder="Pre-shared key"
        :disabled="stream.isStreaming"
        class="mt-3"
      />
    </section>

    <!-- Connection Mode -->
    <section v-if="settings.role === 'source'" class="glass-card">
      <h3 class="text-sm font-semibold text-slate-300 border-b border-white/10 pb-2 mb-4">
        Connection
      </h3>
      <SelectField id="mode-select" v-model="settings.mode" :disabled="stream.isStreaming">
        <option value="client">Client (Send Audio to IP)</option>
        <option value="server">Server (Listen for Connections)</option>
      </SelectField>
      <p class="text-[11px] text-white/50 mt-2 leading-relaxed">
        Client: connect to an audio receiver.<br />
        Server: wait for an audio receiver to connect.
      </p>
    </section>

    <!-- Destination / Server Settings -->
    <section v-if="settings.role === 'source'" class="glass-card">
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
      <InputField
        v-if="settings.isServer"
        id="allowlist-input"
        v-model="settings.allowlist"
        label="Allowlist (optional, IP/CIDR — empty = allow all)"
        placeholder="192.168.1.0/24, 10.0.0.5"
        :disabled="stream.isStreaming"
        class="mt-3"
      />
    </section>
  </div>
</template>

<script setup lang="ts">
import { onMounted } from "vue";
import { useSettingsStore } from "../../stores/settings.ts";
import { useStreamStore } from "../../stores/stream.ts";
import SelectField from "../ui/SelectField.vue";
import InputField from "../ui/InputField.vue";
import CheckboxField from "../ui/CheckboxField.vue";

const settings = useSettingsStore();
const stream = useStreamStore();

onMounted(async () => {
  await settings.loadOutputDevices();
});

async function onLoopbackChange() {
  await settings.loadDevices();
}
</script>
