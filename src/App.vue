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
let mounted = true;

onMounted(async () => {
  document.addEventListener("keydown", onKeyDown);
  settings.initializing = settings.nativeAvailable;
  try {
    await Promise.all([settings.init(), stream.initListeners()]);
    if (!mounted) return;
    settings.initializing = false;
    if (settings.autoStream && !stream.isStreaming) void stream.startStream();
  } catch (error) {
    if (mounted) stream.addToast(String(error), "error");
  }
});

onUnmounted(() => {
  mounted = false;
  document.removeEventListener("keydown", onKeyDown);
  stream.disposeListeners();
});

function onKeyDown(e: KeyboardEvent) {
  if (e.defaultPrevented || e.repeat || e.isComposing || !settings.nativeAvailable) return;
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
    e.preventDefault();
    void stream.saveConfiguration();
  }
  if (
    (e.ctrlKey || e.metaKey) &&
    e.key === "Enter" &&
    !(
      e.target instanceof HTMLInputElement ||
      e.target instanceof HTMLTextAreaElement ||
      (e.target as HTMLElement)?.isContentEditable
    )
  ) {
    e.preventDefault();
    void stream.toggleStream();
  }
}
</script>
