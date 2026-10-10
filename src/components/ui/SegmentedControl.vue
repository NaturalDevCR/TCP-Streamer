<template>
  <div
    class="inline-flex rounded-[var(--radius)] bg-[var(--color-surface-2)] border border-[var(--color-border)] p-0.5"
    role="radiogroup"
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
          ? 'bg-[var(--color-surface-3)] text-[var(--color-text)]'
          : 'text-[var(--color-text-muted)] hover:text-[var(--color-text)]',
      ]"
      :disabled="disabled"
      @keydown="onKey($event, opt.value)"
      @click="$emit('update:modelValue', opt.value)"
    >
      {{ opt.label }}
    </button>
  </div>
</template>
<script setup lang="ts">
const props = defineProps<{
  modelValue?: string | number;
  options: { value: string | number; label: string }[];
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
