<template>
  <nav class="flex gap-1 bg-black/20 p-1.5 rounded-xl mb-4">
    <button
      v-for="tab in tabs"
      :key="tab.id"
      :class="[
        'relative flex-1 flex items-center justify-center py-2.5 rounded-lg cursor-pointer transition-all duration-300 border-0 group',
        modelValue === tab.id
          ? 'text-accent shadow-sm'
          : 'bg-transparent text-slate-400 hover:text-slate-200',
      ]"
      :title="tab.label"
      @click="$emit('update:modelValue', tab.id)"
    >
      <div
        v-if="modelValue === tab.id"
        class="absolute inset-0 bg-white/10 rounded-lg -z-10"
        style="view-transition-name: tab-bubble"
      />
      <component
        :is="tab.icon"
        class="w-5 h-5 transition-transform duration-300 group-hover:scale-110 group-active:scale-95 z-10"
      />
      <!-- Tooltip -->
      <span
        class="absolute -top-8 left-1/2 -translate-x-1/2 px-2 py-1 bg-slate-800 text-xs text-white rounded border border-white/10 opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none whitespace-nowrap z-10"
      >
        {{ tab.label }}
      </span>
    </button>
  </nav>
</template>

<script setup lang="ts">
import { IconConnection, IconAudio, IconSettings, IconAdvanced, IconLogs } from "./icons";

defineProps({ modelValue: String });
defineEmits(["update:modelValue"]);

const tabs = [
  { id: "connection", label: "Connection", icon: IconConnection },
  { id: "audio", label: "Audio", icon: IconAudio },
  { id: "settings", label: "Settings", icon: IconSettings },
  { id: "advanced", label: "Advanced", icon: IconAdvanced },
  { id: "logs", label: "Logs", icon: IconLogs },
];
</script>
