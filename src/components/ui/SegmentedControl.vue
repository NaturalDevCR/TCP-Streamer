<template>
  <div
    :id="field?.id.value"
    class="segmented-control inline-flex rounded-[var(--radius)] bg-[var(--color-surface-2)] border border-[var(--color-border)] p-0.5"
    role="radiogroup"
    :aria-label="field?.label.value"
    :aria-describedby="field?.describedBy.value || undefined"
    :aria-invalid="field?.invalid.value || undefined"
  >
    <button
      v-for="opt in options"
      :key="opt.value"
      role="radio"
      :aria-checked="modelValue === opt.value"
      :tabindex="modelValue === opt.value ? 0 : -1"
      type="button"
      :class="[
        'px-3 py-1.5 text-sm font-medium rounded-[calc(var(--radius)-2px)] transition-colors focus-ring',
        modelValue === opt.value
          ? 'bg-[var(--color-accent-soft)] text-[var(--color-accent)]'
          : 'text-[var(--color-text-muted)] hover:text-[var(--color-text)]',
      ]"
      :disabled="disabled"
      @keydown="onKey($event, opt.value)"
      @click="$emit('update:modelValue', opt.value)"
    >
      <span>{{ opt.label }}</span
      ><small v-if="opt.description">{{ opt.description }}</small>
    </button>
  </div>
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
  options: { value: string | number; label: string; description?: string }[];
  disabled?: boolean;
}>();
const emit = defineEmits<{ "update:modelValue": [string | number] }>();
function onKey(event: KeyboardEvent, value: string | number) {
  if (
    !["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End"].includes(event.key) ||
    props.disabled
  )
    return;
  event.preventDefault();
  const idx = props.options.findIndex((o) => o.value === value);
  const direction = ["ArrowLeft", "ArrowUp"].includes(event.key) ? -1 : 1;
  const next =
    event.key === "Home"
      ? 0
      : event.key === "End"
        ? props.options.length - 1
        : (idx + direction + props.options.length) % props.options.length;
  emit("update:modelValue", props.options[next].value);
  const buttons = (event.currentTarget as HTMLElement).parentElement?.querySelectorAll("button");
  (buttons?.[next] as HTMLElement)?.focus();
}
</script>
