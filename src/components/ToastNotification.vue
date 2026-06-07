<template>
  <Teleport to="body">
    <div class="fixed top-4 right-4 z-50 flex flex-col gap-2 pointer-events-none">
      <TransitionGroup name="toast">
        <div
          v-for="toast in stream.toasts"
          :key="toast.id"
          :class="[
            'pointer-events-auto flex items-center gap-2 px-4 py-2.5 rounded-lg text-sm font-medium shadow-lg border',
            toastStyles[toast.type] || toastStyles.info,
          ]"
        >
          <component :is="toastIcon(toast.type)" class="w-4 h-4 shrink-0" />
          <span>{{ toast.message }}</span>
        </div>
      </TransitionGroup>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { useStreamStore } from "../stores/stream.ts";
import { h } from "vue";

const stream = useStreamStore();

function toastIcon(type: string) {
  const color =
    {
      success: "var(--color-success)",
      error: "var(--color-danger)",
      warning: "var(--color-warning)",
      info: "var(--color-text-muted)",
    }[type] || "var(--color-text-muted)";

  const paths: Record<string, string[]> = {
    success: ["M20 6 9 17l-5-5"],
    error: ["M18 6 6 18", "M6 6l12 12"],
    warning: ["M12 9v4", "M12 17h.01"],
    info: ["M12 16v-4", "M12 8h.01"],
  };

  const p = paths[type] || paths.info;
  return {
    render: () =>
      h(
        "svg",
        {
          xmlns: "http://www.w3.org/2000/svg",
          viewBox: "0 0 24 24",
          fill: "none",
          stroke: color,
          "stroke-width": 2,
          "stroke-linecap": "round",
          "stroke-linejoin": "round",
          width: 16,
          height: 16,
        },
        p.map((d: string) => h("path", { d })),
      ),
  };
}

const toastStyles = {
  success:
    "bg-[var(--color-success)]/10 border-[var(--color-success)]/20 text-[var(--color-success)]",
  error: "bg-[var(--color-danger)]/10 border-[var(--color-danger)]/20 text-[var(--color-danger)]",
  warning:
    "bg-[var(--color-warning)]/10 border-[var(--color-warning)]/20 text-[var(--color-warning)]",
  info: "bg-[var(--color-text-muted)]/10 border-[var(--color-border)] text-[var(--color-text-muted)]",
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
