<template>
  <div class="flex flex-col gap-1.5">
    <label v-if="label" :for="id" class="text-[13px] font-medium text-[var(--color-text-muted)]">
      {{ label }}
    </label>
    <slot :id="id" />
    <p v-if="help" class="text-xs text-[var(--color-text-faint)]">{{ help }}</p>
    <p v-if="error" class="text-xs text-[var(--color-danger)]">{{ error }}</p>
  </div>
</template>
<script setup lang="ts">
defineProps<{ label?: string; help?: string; error?: string; id?: string }>();
</script>
