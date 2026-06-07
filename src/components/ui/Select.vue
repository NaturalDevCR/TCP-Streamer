<template>
  <SelectRoot
    :model-value="modelValue == null ? undefined : String(modelValue)"
    :disabled="disabled"
    @update:model-value="$emit('update:modelValue', $event as string)"
  >
    <SelectTrigger
      class="focus-ring flex w-full h-9 items-center justify-between gap-1 rounded-[var(--radius)] bg-[var(--color-surface-1)] border border-[var(--color-border)] px-3 text-sm text-[var(--color-text)] data-[disabled]:opacity-50 data-[disabled]:cursor-not-allowed hover:border-[var(--color-border-strong)]"
    >
      <SelectValue :placeholder="placeholder" />
      <IconChevronDown class="w-4 h-4 text-[var(--color-text-muted)] shrink-0" />
    </SelectTrigger>
    <SelectPortal>
      <SelectContent
        position="popper"
        :side-offset="4"
        class="z-50 min-w-[var(--reka-select-trigger-width)] overflow-hidden rounded-[var(--radius)] bg-[var(--color-surface-2)] border border-[var(--color-border)] shadow-lg"
      >
        <SelectViewport class="p-1 max-h-60 overflow-y-auto">
          <slot />
        </SelectViewport>
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
  SelectViewport,
} from "reka-ui";
import { IconChevronDown } from "../icons";
defineProps<{ modelValue?: string | number; placeholder?: string; disabled?: boolean }>();
defineEmits<{ "update:modelValue": [string | number] }>();
</script>
