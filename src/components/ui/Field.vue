<template>
  <div class="flex flex-col gap-1.5">
    <SettingLabel v-if="label" :label="label" :for-id="fieldId" :tooltip="tooltip" as-label />
    <slot :id="fieldId" />
    <p v-if="help" :id="`${fieldId}-help`" class="text-xs text-[var(--color-text-faint)]">
      {{ help }}
    </p>
    <p
      v-if="error"
      :id="`${fieldId}-error`"
      role="alert"
      class="text-xs text-[var(--color-danger)]"
    >
      {{ error }}
    </p>
  </div>
</template>
<script setup lang="ts">
import SettingLabel from "./SettingLabel.vue";

import { computed, provide, useId } from "vue";
const props = defineProps<{
  label?: string;
  tooltip?: string;
  help?: string;
  error?: string;
  id?: string;
}>();
const generatedId = useId();
const fieldId = computed(() => props.id || generatedId);
provide("field", {
  id: fieldId,
  label: computed(() => props.label),
  describedBy: computed(() =>
    [props.help && `${fieldId.value}-help`, props.error && `${fieldId.value}-error`]
      .filter(Boolean)
      .join(" "),
  ),
  invalid: computed(() => Boolean(props.error)),
});
</script>
