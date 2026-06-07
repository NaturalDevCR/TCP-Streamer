<template>
  <span
    :class="['inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium', toneClass]"
  >
    <slot />
  </span>
</template>
<script setup lang="ts">
import { computed } from "vue";
const props = defineProps<{ tone?: "success" | "warning" | "danger" | "neutral" }>();
const toneClass = computed(() => {
  switch (props.tone) {
    case "success":
      return "bg-[var(--color-success)]/15 text-[var(--color-success)]";
    case "warning":
      return "bg-[var(--color-warning)]/15 text-[var(--color-warning)]";
    case "danger":
      return "bg-[var(--color-danger)]/15 text-[var(--color-danger)]";
    default:
      return "bg-[var(--color-surface-3)] text-[var(--color-text-muted)]";
  }
});
</script>
