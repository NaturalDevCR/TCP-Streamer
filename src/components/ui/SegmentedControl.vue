<template>
  <div
    class="inline-flex rounded-[var(--radius)] bg-[var(--color-surface-2)] border border-[var(--color-border)] p-0.5"
    role="radiogroup"
  >
    <button
      v-for="opt in options"
      :key="opt.value"
      type="button"
      :class="[
        'px-3 py-1.5 text-sm font-medium rounded-[calc(var(--radius)-2px)] transition-colors focus-ring',
        modelValue === opt.value
          ? 'bg-[var(--color-surface-3)] text-[var(--color-text)]'
          : 'text-[var(--color-text-muted)] hover:text-[var(--color-text)]',
      ]"
      :disabled="disabled"
      @click="$emit('update:modelValue', opt.value)"
    >
      {{ opt.label }}
    </button>
  </div>
</template>
<script setup lang="ts">
defineProps<{
  modelValue?: string | number;
  options: { value: string | number; label: string }[];
  disabled?: boolean;
}>();
defineEmits<{ "update:modelValue": [string | number] }>();
</script>
