<template>
  <Teleport to="body">
    <div class="fixed top-4 right-4 z-50 flex flex-col gap-2 pointer-events-none">
      <TransitionGroup name="toast">
        <div
          v-for="toast in stream.toasts"
          :key="toast.id"
          :class="[
            'pointer-events-auto flex items-center gap-2 px-4 py-2.5 rounded-lg text-sm font-medium shadow-lg backdrop-blur-sm border',
            toastStyles[toast.type] || toastStyles.info
          ]"
        >
          <span class="font-bold text-base">{{ icons[toast.type] || 'ℹ' }}</span>
          <span>{{ toast.message }}</span>
        </div>
      </TransitionGroup>
    </div>
  </Teleport>
</template>

<script setup>
import { useStreamStore } from "../stores/stream.js";

const stream = useStreamStore();

const icons = { success: "✓", error: "✕", warning: "⚠", info: "ℹ" };

const toastStyles = {
  success: "bg-emerald-500/20 border-emerald-500/30 text-emerald-300",
  error: "bg-red-500/20 border-red-500/30 text-red-300",
  warning: "bg-amber-500/20 border-amber-500/30 text-amber-300",
  info: "bg-blue-500/20 border-blue-500/30 text-blue-300",
};
</script>

<style scoped>
.toast-enter-active {
  animation: toast-in 0.3s ease-out;
}
.toast-leave-active {
  animation: toast-out 0.3s ease-in forwards;
}
</style>
