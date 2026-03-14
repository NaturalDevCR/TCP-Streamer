<template>
  <div class="animate-fade-in space-y-4">
    <!-- Advanced Network -->
    <section class="glass-card">
      <h3 class="text-sm font-semibold text-slate-300 border-b border-white/10 pb-2 mb-4">Advanced Network</h3>
      <div class="space-y-4">
        <CheckboxField v-model="settings.highPriority" label="High Priority Thread" />

        <SelectField v-model="settings.dscpStrategy" label="DSCP Strategy" id="dscp-select">
          <option value="voip">VoIP (EF - 0xB8)</option>
          <option value="lowdelay">Low Delay (0x10)</option>
          <option value="throughput">Throughput (0x08)</option>
          <option value="besteffort">Best Effort (0x00)</option>
        </SelectField>

        <SelectField v-model.number="settings.chunkSize" label="Chunk Size (Samples)" id="chunk-size-select">
          <option :value="128">128 (Ultra Low Latency)</option>
          <option :value="256">256 (Very Low Latency)</option>
          <option :value="512">512 (Low Latency)</option>
          <option :value="1024">1024 (Stable)</option>
          <option :value="2048">2048 (High Stability)</option>
          <option :value="4096">4096 (Max Stability)</option>
        </SelectField>
      </div>
    </section>

    <!-- Network Presets -->
    <section class="glass-card">
      <h3 class="text-sm font-semibold text-slate-300 border-b border-white/10 pb-2 mb-4">Network Presets</h3>
      <SelectField
        v-model="settings.networkPreset"
        label="Connection Type"
        id="network-preset"
        @update:modelValue="onPresetChange"
      >
        <option value="custom">Custom</option>
        <option value="ethernet">Ethernet (Stable)</option>
        <option value="wifi">WiFi (Standard)</option>
        <option value="wifi-poor">WiFi (Poor Signal)</option>
      </SelectField>
      <p class="text-[11px] text-white/50 mt-2">Optimized settings for your connection type</p>
    </section>
  </div>
</template>

<script setup>
import { useSettingsStore } from "../../stores/settings.js";
import { useStreamStore } from "../../stores/stream.js";
import SelectField from "../ui/SelectField.vue";
import CheckboxField from "../ui/CheckboxField.vue";

const settings = useSettingsStore();
const stream = useStreamStore();

function onPresetChange(presetId) {
  if (presetId !== "custom") {
    settings.applyPreset(presetId);
    const names = { ethernet: "Ethernet (Stable)", wifi: "WiFi (Standard)", "wifi-poor": "WiFi (Poor Signal)" };
    stream.addToast(`Applied ${names[presetId]} preset`, "success");
  }
}
</script>
