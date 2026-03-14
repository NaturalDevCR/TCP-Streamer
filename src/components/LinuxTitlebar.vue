<template>
  <div
    v-if="showTitlebar"
    class="fixed top-0 left-0 right-0 h-9 bg-slate-950/95 border-b border-white/8 z-[9999] flex items-center justify-between px-2 pl-4 select-none"
  >
    <div
      @mousedown="startDrag"
      class="flex-1 h-full flex items-center cursor-grab active:cursor-grabbing"
    >
      <span class="text-xs font-medium text-white/60 pointer-events-none">TCP Streamer</span>
    </div>
    <div class="flex gap-1 items-center">
      <button
        @click="minimize"
        class="w-8 h-7 border-0 bg-transparent text-white/60 text-sm cursor-pointer rounded flex items-center justify-center hover:bg-white/10 hover:text-white transition-all"
      >─</button>
      <button
        @click="close"
        class="w-8 h-7 border-0 bg-transparent text-white/60 text-sm cursor-pointer rounded flex items-center justify-center hover:bg-red-500/80 hover:text-white transition-all"
      >✕</button>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted } from "vue";
import { getCurrentWindow, invoke } from "../composables/useTauri.js";
import { useSettingsStore } from "../stores/settings.js";

const settings = useSettingsStore();
const showTitlebar = ref(false);

onMounted(async () => {
  if (settings.osType === "linux") {
    showTitlebar.value = true;
  }
});

const appWindow = getCurrentWindow();

function startDrag(e) {
  if (e.button === 0) appWindow.startDragging();
}
function minimize() { appWindow.minimize(); }
function close() { appWindow.close(); }
</script>
