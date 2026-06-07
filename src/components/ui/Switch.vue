<template>
  <SwitchRoot
    :model-value="modelValue"
    :disabled="disabled"
    class="focus-ring relative inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-full transition-colors data-[state=checked]:bg-[var(--color-accent)] bg-[var(--color-surface-3)]"
    @update:model-value="$emit('update:modelValue', $event)"
  >
    <SwitchThumb
      class="block h-4 w-4 rounded-full bg-white shadow-sm transition-transform data-[state=checked]:translate-x-[18px] translate-x-[2px]"
    />
  </SwitchRoot>
</template>
<script setup lang="ts">
import { SwitchRoot, SwitchThumb } from "reka-ui";
defineProps<{ modelValue?: boolean; disabled?: boolean }>();
defineEmits<{ "update:modelValue": [boolean] }>();
</script>
