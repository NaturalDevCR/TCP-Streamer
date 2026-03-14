<template>
  <nav class="flex gap-1 bg-black/20 p-1.5 rounded-xl mb-4">
    <button
      v-for="tab in tabs"
      :key="tab.id"
      @click="$emit('update:modelValue', tab.id)"
      :class="[
        'relative flex-1 flex items-center justify-center py-2.5 rounded-lg cursor-pointer transition-all duration-200 border-0 group',
        modelValue === tab.id
          ? 'bg-white/10 text-accent shadow-sm'
          : 'bg-transparent text-slate-400 hover:bg-white/5 hover:text-slate-200'
      ]"
      :title="tab.label"
    >
      <component :is="tab.icon" class="w-5 h-5 transition-transform group-hover:scale-110" />
      <!-- Tooltip -->
      <span class="absolute -top-8 left-1/2 -translate-x-1/2 px-2 py-1 bg-slate-800 text-xs text-white rounded border border-white/10 opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none whitespace-nowrap z-10">
        {{ tab.label }}
      </span>
    </button>
  </nav>
</template>

<script setup>
import { h } from "vue";

defineProps({ modelValue: String });
defineEmits(["update:modelValue"]);

// SVG icon components
const IconConnection = { render: () => h('svg', { xmlns: 'http://www.w3.org/2000/svg', viewBox: '0 0 24 24', fill: 'none', stroke: 'currentColor', 'stroke-width': 2, 'stroke-linecap': 'round', 'stroke-linejoin': 'round' }, [h('path', { d: 'M12 20h.01' }), h('path', { d: 'M2 20h.01' }), h('path', { d: 'M22 20h.01' }), h('path', { d: 'M5 12a7 7 0 0 1 14 0' }), h('path', { d: 'M8.5 15.5a3.5 3.5 0 0 1 7 0' })]) };
const IconAudio = { render: () => h('svg', { xmlns: 'http://www.w3.org/2000/svg', viewBox: '0 0 24 24', fill: 'none', stroke: 'currentColor', 'stroke-width': 2, 'stroke-linecap': 'round', 'stroke-linejoin': 'round' }, [h('path', { d: 'M11 5L6 9H2v6h4l5 4V5z' }), h('path', { d: 'M19.07 4.93a10 10 0 0 1 0 14.14M15.54 8.46a5 5 0 0 1 0 7.07' })]) };
const IconSettings = { render: () => h('svg', { xmlns: 'http://www.w3.org/2000/svg', viewBox: '0 0 24 24', fill: 'none', stroke: 'currentColor', 'stroke-width': 2, 'stroke-linecap': 'round', 'stroke-linejoin': 'round' }, [h('path', { d: 'M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.1a2 2 0 0 1-1-1.72v-.51a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z' }), h('circle', { cx: 12, cy: 12, r: 3 })]) };
const IconAdvanced = { render: () => h('svg', { xmlns: 'http://www.w3.org/2000/svg', viewBox: '0 0 24 24', fill: 'none', stroke: 'currentColor', 'stroke-width': 2, 'stroke-linecap': 'round', 'stroke-linejoin': 'round' }, [h('path', { d: 'M12 3v18' }), h('path', { d: 'M3 12h18' }), h('path', { d: 'M7 7l10 10' }), h('path', { d: 'M17 7L7 17' })]) };
const IconLogs = { render: () => h('svg', { xmlns: 'http://www.w3.org/2000/svg', viewBox: '0 0 24 24', fill: 'none', stroke: 'currentColor', 'stroke-width': 2, 'stroke-linecap': 'round', 'stroke-linejoin': 'round' }, [h('path', { d: 'M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z' }), h('polyline', { points: '14 2 14 8 20 8' }), h('line', { x1: 16, y1: 13, x2: 8, y2: 13 }), h('line', { x1: 16, y1: 17, x2: 8, y2: 17 })]) };

const tabs = [
  { id: "connection", label: "Connection", icon: IconConnection },
  { id: "audio", label: "Audio", icon: IconAudio },
  { id: "settings", label: "Settings", icon: IconSettings },
  { id: "advanced", label: "Advanced", icon: IconAdvanced },
  { id: "logs", label: "Logs", icon: IconLogs },
];
</script>
