<template>
  <div class="animate-fade-in space-y-4">
    <!-- Advanced Network -->
    <section class="glass-card">
      <h3 class="text-sm font-semibold text-slate-300 border-b border-white/10 pb-2 mb-4">
        Advanced Network
      </h3>
      <div class="space-y-4">
        <CheckboxField v-model="settings.highPriority" label="High Priority Thread" />

        <SelectField id="dscp-select" v-model="settings.dscpStrategy" label="DSCP Strategy">
          <option value="voip">VoIP (EF - 0xB8)</option>
          <option value="lowdelay">Low Delay (0x10)</option>
          <option value="throughput">Throughput (0x08)</option>
          <option value="besteffort">Best Effort (0x00)</option>
        </SelectField>

        <SelectField
          id="chunk-size-select"
          v-model.number="settings.chunkSize"
          label="Chunk Size (Samples)"
        >
          <option :value="128">128 (Ultra Low Latency)</option>
          <option :value="256">256 (Very Low Latency)</option>
          <option :value="512">512 (Low Latency)</option>
          <option :value="1024">1024 (Stable)</option>
          <option :value="2048">2048 (High Stability)</option>
          <option :value="4096">4096 (Max Stability)</option>
        </SelectField>
      </div>
    </section>

    <!-- Latency Profile -->
    <section class="glass-card">
      <h3 class="border-b border-white/10 pb-2 mb-4 text-sm font-semibold text-slate-300">
        Latency Profile
      </h3>
      <SelectField
        id="latency-profile"
        v-model="settings.latencyProfile"
        label="Latency vs Robustness"
      >
        <option value="ultra-low">Ultra-low latency</option>
        <option value="balanced">Balanced</option>
        <option value="robust">Robust (high jitter tolerance)</option>
        <option value="custom">Custom (use Audio tab buffers)</option>
      </SelectField>
      <p class="mt-2 text-[11px] text-white/50">
        Lower latency uses smaller buffers (best on wired/quiet networks). Loopback capture needs
        more buffering &mdash; ultra-low may stutter.
      </p>
    </section>
  </div>
</template>

<script setup lang="ts">
import { useSettingsStore } from "../../stores/settings.ts";
import SelectField from "../ui/SelectField.vue";
import CheckboxField from "../ui/CheckboxField.vue";

const settings = useSettingsStore();
</script>
