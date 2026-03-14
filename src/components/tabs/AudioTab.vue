<template>
  <div class="animate-fade-in space-y-4">
    <!-- Audio Configuration -->
    <section class="glass-card">
      <h3 class="text-sm font-semibold text-slate-300 border-b border-white/10 pb-2 mb-4">Audio Configuration</h3>
      <div class="flex gap-3">
        <SelectField v-model.number="settings.sampleRate" label="Sample Rate" id="sample-rate" :disabled="stream.isStreaming">
          <option :value="48000">48 kHz</option>
          <option :value="44100">44.1 kHz</option>
        </SelectField>
        <SelectField v-model.number="settings.bufferSize" label="Buffer Size" id="buffer-size" :disabled="stream.isStreaming">
          <option :value="256">Low (256)</option>
          <option :value="512">Medium (512)</option>
          <option :value="1024">High (1024)</option>
          <option :value="2048">Max (2048)</option>
        </SelectField>
      </div>
      <div class="flex gap-3 mt-3">
        <SelectField v-model.number="settings.ringBufferDuration" label="Ring Buffer (ms)" id="ring-buffer-duration" :disabled="stream.isStreaming">
          <option :value="2000">Low (2000ms)</option>
          <option :value="4000">Balanced (4000ms)</option>
          <option :value="8000">High (8000ms)</option>
          <option :value="15000">Max (15000ms)</option>
        </SelectField>
        <SelectField
          v-if="settings.isServer"
          v-model="settings.format"
          label="Stream Format"
          id="format-select"
          :disabled="stream.isStreaming"
        >
          <option value="pcm">PCM (Raw / Snapserver)</option>
          <option value="wav">WAV (Header / Browser)</option>
        </SelectField>
      </div>
    </section>

    <!-- Server Stream URLs -->
    <section
      v-if="settings.isServer && stream.isStreaming"
      class="glass-card border-emerald-500/20 bg-emerald-500/[0.03]"
    >
      <h3 class="text-sm font-semibold text-emerald-400 flex items-center gap-2 mb-4">
        <span class="text-lg leading-none animate-pulse-slow">●</span> Server Running
      </h3>
      <div class="space-y-3">
        <CopyField v-model="stream.tcpUrl" label="Snapcast Server Address" input-class="text-emerald-400 font-semibold" />
        <CopyField v-model="stream.httpUrl" label="Browser Stream URL" />
        <div class="flex flex-col gap-1.5">
          <label class="text-xs font-medium text-slate-400 ml-0.5">Snapserver Config (snapserver.conf)</label>
          <div class="flex gap-2 items-stretch">
            <textarea
              :value="stream.snapcastConfig"
              readonly
              class="flex-1 h-[70px] font-mono text-[11px] bg-black/30 border border-white/10 text-white/60 resize-none p-2 rounded-lg leading-relaxed"
            />
            <button
              @click="copyConfig"
              class="w-9 flex items-center justify-center bg-white/10 border border-white/10 rounded-lg text-white/80 hover:bg-white/20 transition-all cursor-pointer text-sm"
              title="Copy Config"
            >📋</button>
          </div>
        </div>
      </div>
    </section>

    <!-- Adaptive Buffer -->
    <section class="glass-card">
      <h3 class="text-sm font-semibold text-accent border-b border-white/10 pb-2 mb-4">Adaptive Buffer</h3>
      <CheckboxField
        v-model="settings.adaptiveBuffer"
        label="Enable Adaptive Buffer Sizing"
        description="Automatically adjust buffer size based on network jitter"
      />
      <div class="flex gap-3 mt-4">
        <InputField v-model.number="settings.minBuffer" label="Min Buffer (ms)" id="min-buffer" type="number" :min="1000" :max="5000" :step="500" />
        <InputField v-model.number="settings.maxBuffer" label="Max Buffer (ms)" id="max-buffer" type="number" :min="5000" :max="20000" :step="1000" />
      </div>
    </section>
  </div>
</template>

<script setup>
import { useSettingsStore } from "../../stores/settings.js";
import { useStreamStore } from "../../stores/stream.js";
import SelectField from "../ui/SelectField.vue";
import InputField from "../ui/InputField.vue";
import CheckboxField from "../ui/CheckboxField.vue";
import CopyField from "../ui/CopyField.vue";

const settings = useSettingsStore();
const stream = useStreamStore();

function copyConfig() {
  if (stream.snapcastConfig) {
    navigator.clipboard.writeText(stream.snapcastConfig)
      .then(() => stream.addToast("Config copied!", "success"))
      .catch(() => stream.addToast("Failed to copy", "error"));
  }
}
</script>
