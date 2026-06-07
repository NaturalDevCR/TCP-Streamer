<template>
  <span class="flex items-center gap-1.5">
    <SelectItem
      :value="String(value)"
      class="relative flex w-full cursor-pointer select-none items-center rounded-sm px-2 py-1.5 text-sm text-[var(--color-text)] outline-none hover:bg-[var(--color-accent)] hover:text-[var(--color-on-accent)] data-[highlighted]:bg-[var(--color-accent)] data-[highlighted]:text-[var(--color-on-accent)]"
    >
      <SelectItemText>{{ label }}</SelectItemText>
    </SelectItem>
  </span>
</template>
<script setup lang="ts">
import { SelectItem, SelectItemText } from "reka-ui";
defineProps<{ value: string | number; label: string }>();
</script>
