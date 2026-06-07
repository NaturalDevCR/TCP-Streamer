<template>
  <input
    :type="type ?? 'text'"
    :value="modelValue"
    :placeholder="placeholder"
    :disabled="disabled"
    class="focus-ring w-full h-9 px-3 rounded-[var(--radius)] bg-[var(--color-surface-1)] border border-[var(--color-border)] text-sm text-[var(--color-text)] placeholder:text-[var(--color-text-faint)] disabled:opacity-50"
    @input="onInput"
  />
</template>
<script setup lang="ts">
const props = defineProps<{
  modelValue?: string | number;
  type?: string;
  placeholder?: string;
  disabled?: boolean;
}>();
const emit = defineEmits<{ "update:modelValue": [string | number] }>();
function onInput(e: Event) {
  const v = (e.target as HTMLInputElement).value;
  emit("update:modelValue", props.type === "number" ? Number(v) : v);
}
</script>
