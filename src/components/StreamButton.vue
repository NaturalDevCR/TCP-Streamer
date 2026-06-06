<template>
  <div class="mt-2">
    <button
      :class="[
        'glow-btn w-full py-3 border-0 rounded-xl text-white font-semibold text-[15px] cursor-pointer transition-all duration-500 hover:scale-[1.01] active:scale-[0.98]',
        stream.isStreaming ? 'stop bg-red-500 hover:bg-red-400' : 'bg-accent hover:bg-blue-400',
      ]"
      @click="stream.toggleStream"
    >
      {{ buttonText }}
    </button>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useStreamStore } from "../stores/stream.ts";
import { useSettingsStore } from "../stores/settings.ts";

const stream = useStreamStore();
const settings = useSettingsStore();

const buttonText = computed(() => {
  if (stream.isStreaming) {
    return settings.isServer ? "Stop Listening" : "Stop Streaming";
  }
  return settings.isServer ? "Start Listening" : "Start Streaming";
});
</script>
