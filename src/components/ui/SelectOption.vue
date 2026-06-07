<template>
  <SelectItem
    :value="String(value)"
    class="relative flex w-full cursor-pointer select-none items-center rounded-[6px] px-2 py-1.5 text-sm text-[var(--color-text)] outline-none data-[highlighted]:bg-[var(--color-accent)] data-[highlighted]:text-[var(--color-on-accent)]"
  >
    <SelectItemText>{{ label }}</SelectItemText>
  </SelectItem>
</template>
<script setup lang="ts">
import { SelectItem, SelectItemText } from "reka-ui";
defineProps<{ value: string | number; label: string }>();
</script>
