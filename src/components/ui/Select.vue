<template>
  <SelectRoot
    :model-value="modelValue"
    :disabled="disabled"
    @update:model-value="$emit('update:modelValue', $event)"
  >
    <SelectTrigger
      class="focus-ring flex w-full h-9 items-center justify-between gap-1 rounded-[var(--radius)] bg-[var(--color-surface-1)] border border-[var(--color-border)] px-3 text-sm text-[var(--color-text)] disabled:opacity-50 disabled:cursor-not-allowed hover:border-[var(--color-border-strong)]"
    >
      <SelectValue :placeholder="placeholder" />
      <IconChevronDown class="w-4 h-4 text-[var(--color-text-muted)] shrink-0" />
    </SelectTrigger>
    <SelectPortal>
      <SelectContent
        :side-offset="4"
        class="z-50 min-w-[8rem] overflow-hidden rounded-[var(--radius)] bg-[var(--color-surface-2)] border border-[var(--color-border)] shadow-lg"
      >
        <SelectArrow class="text-red-500" />
        <slot />
      </SelectContent>
    </SelectPortal>
  </SelectRoot>
</template>
<script setup lang="ts">
import {
  SelectRoot,
  SelectTrigger,
  SelectContent,
  SelectValue,
  SelectPortal,
  SelectArrow,
} from "reka-ui";
import { IconChevronDown } from "../icons";
defineProps<{ modelValue?: string | number; placeholder?: string; disabled?: boolean }>();
defineEmits<{ "update:modelValue": [string | number] }>();
</script>
