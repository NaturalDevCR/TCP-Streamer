<template>
  <textarea
    :value="modelValue"
    :readonly="readonly"
    :disabled="disabled"
    :placeholder="placeholder"
    class="focus-ring w-full px-3 py-2 rounded-[var(--radius)] bg-[var(--color-surface-1)] border border-[var(--color-border)] font-mono text-sm text-[var(--color-text)] placeholder:text-[var(--color-text-faint)] resize-none disabled:opacity-50"
    :rows="rows"
    @input="onInput"
  />
</template>
<script setup lang="ts">
defineProps<{
  modelValue?: string;
  readonly?: boolean;
  disabled?: boolean;
  placeholder?: string;
  rows?: number;
}>();
const emit = defineEmits<{ "update:modelValue": [string] }>();
function onInput(e: Event) {
  emit("update:modelValue", (e.target as HTMLTextAreaElement).value);
}
</script>
