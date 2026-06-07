<template>
  <button
    :type="type"
    :disabled="disabled"
    :class="[
      'focus-ring inline-flex items-center justify-center gap-2 rounded-[var(--radius)] font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed',
      sizeClass,
      variantClass,
    ]"
  >
    <slot />
  </button>
</template>
<script setup lang="ts">
import { computed } from "vue";
const props = defineProps<{
  variant?: "primary" | "secondary" | "ghost" | "danger";
  size?: "sm" | "md";
  type?: "button" | "submit";
  disabled?: boolean;
}>();
const sizeClass = computed(() =>
  props.size === "sm" ? "h-8 px-3 text-[13px]" : "h-9 px-4 text-sm",
);
const variantClass = computed(() => {
  switch (props.variant) {
    case "secondary":
      return "bg-[var(--color-surface-2)] text-[var(--color-text)] border border-[var(--color-border)] hover:bg-[var(--color-surface-3)]";
    case "ghost":
      return "bg-transparent text-[var(--color-text-muted)] hover:bg-[var(--color-surface-2)] hover:text-[var(--color-text)]";
    case "danger":
      return "bg-[var(--color-danger)] text-white hover:opacity-90";
    default:
      return "bg-[var(--color-accent)] text-[var(--color-on-accent)] hover:bg-[var(--color-accent-hover)]";
  }
});
</script>
