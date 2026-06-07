<template>
  <span class="inline-flex items-center gap-1.5">
    <span :class="['inline-block w-2 h-2 rounded-full shrink-0', colorClass]" />
    <span v-if="label" class="text-sm text-[var(--color-text)]">{{ label }}</span>
  </span>
</template>
<script setup lang="ts">
import { computed } from "vue";
const props = defineProps<{
  tone?: "success" | "warning" | "danger" | "neutral";
  label?: string;
}>();
const colorClass = computed(() => {
  switch (props.tone) {
    case "success":
      return "bg-[var(--color-success)]";
    case "warning":
      return "bg-[var(--color-warning)]";
    case "danger":
      return "bg-[var(--color-danger)]";
    default:
      return "bg-[var(--color-text-muted)]";
  }
});
</script>
