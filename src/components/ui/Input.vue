<template>
  <input
    :id="field?.id.value"
    :aria-label="field?.label.value"
    :aria-describedby="field?.describedBy.value || undefined"
    :aria-invalid="field?.invalid.value || undefined"
    :type="type ?? 'text'"
    :value="modelValue"
    :placeholder="placeholder"
    :disabled="disabled"
    class="focus-ring w-full h-9 px-3 rounded-[var(--radius)] bg-[var(--color-surface-1)] border border-[var(--color-border)] text-sm text-[var(--color-text)] placeholder:text-[var(--color-text-faint)] disabled:opacity-50"
    @input="onInput"
  />
</template>
<script setup lang="ts">
import { inject, type ComputedRef } from "vue";
const field = inject<
  | {
      id: ComputedRef<string>;
      label: ComputedRef<string | undefined>;
      describedBy: ComputedRef<string>;
      invalid: ComputedRef<boolean>;
    }
  | undefined
>("field", undefined);
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
