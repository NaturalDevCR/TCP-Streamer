<template>
  <div
    class="rounded-[var(--radius)] bg-[var(--color-surface-2)] border border-[var(--color-border)] px-3 py-2.5 flex flex-col gap-1"
  >
    <span class="text-[10px] uppercase tracking-wide text-[var(--color-text-faint)]">
      {{ label }}
    </span>
    <span class="font-mono text-sm text-[var(--color-text)]"
      ><slot>{{ value }}</slot></span
    >
  </div>
</template>
<script setup lang="ts">
defineProps<{ label?: string; value?: string | number }>();
</script>
