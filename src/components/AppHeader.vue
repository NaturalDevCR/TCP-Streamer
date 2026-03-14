<template>
  <header class="flex items-center justify-between mb-2">
    <div class="flex items-center gap-3">
      <img src="/assets/logo.png" alt="TCP Streamer" class="w-8 h-8 object-contain drop-shadow-[0_0_8px_rgba(59,130,246,0.5)]" />
      <h1 class="text-lg font-semibold bg-gradient-to-r from-white to-slate-400 bg-clip-text text-transparent">TCP Streamer</h1>
    </div>
    <div
      :class="[
        'flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-medium border transition-all duration-300',
        stream.isStreaming
          ? 'text-emerald-400 bg-emerald-500/10 border-emerald-500/20'
          : 'text-slate-400 bg-black/20 border-white/5'
      ]"
    >
      <span
        :class="[
          'w-1.5 h-1.5 rounded-full transition-all duration-300',
          stream.isStreaming
            ? 'bg-emerald-400 shadow-[0_0_8px_rgba(16,185,129,0.6)]'
            : 'bg-slate-500'
        ]"
      />
      <span>{{ stream.statusText }}</span>
    </div>
  </header>
</template>

<script setup>
import { useStreamStore } from "../stores/stream.js";
const stream = useStreamStore();
</script>
