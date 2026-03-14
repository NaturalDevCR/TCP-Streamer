<template>
  <div class="mt-2">
    <button
      @click="stream.toggleStream"
      :class="[
        'glow-btn w-full py-3 border-0 rounded-xl text-white font-semibold text-[15px] cursor-pointer',
        stream.isStreaming ? 'stop bg-red-500' : 'bg-accent'
      ]"
    >
      {{ buttonText }}
    </button>
  </div>
</template>

<script setup>
import { computed } from "vue";
import { useStreamStore } from "../stores/stream.js";
import { useSettingsStore } from "../stores/settings.js";

const stream = useStreamStore();
const settings = useSettingsStore();

const buttonText = computed(() => {
  if (stream.isStreaming) {
    return settings.isServer ? "Stop Listening" : "Stop Streaming";
  }
  return settings.isServer ? "Start Listening" : "Start Streaming";
});
</script>
