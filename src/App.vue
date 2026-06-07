<template>
  <AppShell />
</template>

<script setup lang="ts">
import { onMounted, onUnmounted } from "vue";
import { useSettingsStore } from "./stores/settings.ts";
import { useStreamStore } from "./stores/stream.ts";
import AppShell from "./components/AppShell.vue";

const settings = useSettingsStore();
const stream = useStreamStore();

onMounted(async () => {
  await settings.init();
  await stream.initListeners();

  if (settings.autoStream && !stream.isStreaming) {
    setTimeout(() => stream.startStream(), 500);
  }

  document.addEventListener("keydown", onKeyDown);
});

onUnmounted(() => {
  document.removeEventListener("keydown", onKeyDown);
});

function onKeyDown(e: KeyboardEvent) {
  if ((e.ctrlKey || e.metaKey) && e.key === "s") {
    e.preventDefault();
    stream.toggleStream();
  }
}

window.addEventListener("beforeunload", (e) => {
  if (stream.isStreaming) {
    e.preventDefault();
    e.returnValue = "Streaming is active. Are you sure you want to close?";
  }
});

window.addEventListener("unload", () => {
  if (stream.isStreaming) {
    stream.stopStream();
  }
});
</script>
