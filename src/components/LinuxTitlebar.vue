<template>
  <div
    v-if="settings.osType === 'linux'"
    data-tauri-drag-region
    class="fixed top-0 left-0 w-full h-10 bg-base border-b border-white/5 z-50 flex items-center justify-between px-3 select-none"
    @mousedown="startDrag"
  >
    <div class="flex items-center gap-2 pointer-events-none">
      <img src="/assets/tcp-streamer-logo.svg" alt="Logo" class="w-4 h-4" />
      <span class="text-white/80 text-xs font-semibold tracking-wide">TCP Streamer</span>
    </div>
    
    <div class="flex gap-1 items-center">
      <button
        @click="minimize"
        class="w-8 h-7 border-0 bg-transparent text-white/60 text-sm cursor-pointer rounded flex items-center justify-center hover:bg-white/10 hover:text-white transition-all"
        title="Minimize"
      >─</button>
      <button
        @click="maximize"
        class="w-8 h-7 border-0 bg-transparent text-white/60 text-sm cursor-pointer rounded flex items-center justify-center hover:bg-white/10 hover:text-white transition-all"
        title="Maximize"
      >□</button>
      <button
        @click="close"
        class="w-8 h-7 border-0 bg-transparent text-white/60 text-sm cursor-pointer rounded flex items-center justify-center hover:bg-red-500/80 hover:text-white transition-all"
        title="Close"
      >✕</button>
    </div>
  </div>
</template>

<script setup>
import { useSettingsStore } from "../stores/settings.js";
import { getCurrentWindow } from '@tauri-apps/api/window';

const settings = useSettingsStore();
const appWindow = getCurrentWindow();

function startDrag(e) {
  if (e.button === 0) appWindow.startDragging();
}
function minimize() { appWindow.minimize(); }
function maximize() { appWindow.toggleMaximize(); }
function close() { appWindow.close(); }
</script>
