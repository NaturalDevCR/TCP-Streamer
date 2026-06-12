<template>
  <div class="flex flex-col gap-1.5">
    <SettingLabel v-if="label" :label="label" :for-id="id" :tooltip="tooltip" as-label />
    <slot :id="id" />
    <p v-if="help" class="text-xs text-[var(--color-text-faint)]">{{ help }}</p>
    <p v-if="error" class="text-xs text-[var(--color-danger)]">{{ error }}</p>
  </div>
</template>
<script setup lang="ts">
import SettingLabel from "./SettingLabel.vue";

defineProps<{
  label?: string;
  tooltip?: string;
  help?: string;
  error?: string;
  id?: string;
}>();
</script>
