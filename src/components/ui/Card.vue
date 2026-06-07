<template>
  <section
    class="rounded-[var(--radius)] bg-[var(--color-surface-1)] border border-[var(--color-border)]"
  >
    <header
      v-if="title || $slots.header"
      class="px-4 py-3 border-b border-[var(--color-border)] flex items-center justify-between"
    >
      <h3 class="text-sm font-semibold text-[var(--color-text)]">{{ title }}</h3>
      <slot name="header" />
    </header>
    <div class="p-4"><slot /></div>
  </section>
</template>
<script setup lang="ts">
defineProps<{ title?: string }>();
</script>
