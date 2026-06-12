<template>
  <div class="inline-flex items-center gap-1.5">
    <label
      v-if="asLabel || forId"
      :for="forId"
      class="text-[13px] font-medium text-[var(--color-text-muted)]"
    >
      {{ label }}
    </label>
    <span v-else class="text-sm text-[var(--color-text)]">{{ label }}</span>

    <Tooltip v-if="tooltip">
      <template #trigger>
        <button
          type="button"
          class="focus-ring inline-flex h-4 w-4 shrink-0 items-center justify-center rounded-full border border-[var(--color-border-strong)] text-[10px] font-semibold leading-none text-[var(--color-text-muted)] hover:border-[var(--color-accent)] hover:text-[var(--color-text)]"
          :aria-label="t('tooltip.moreInfo', { option: label })"
        >
          <span aria-hidden="true">i</span>
        </button>
      </template>

      <span class="block max-w-64 leading-relaxed">{{ tooltip }}</span>
    </Tooltip>
  </div>
</template>

<script setup lang="ts">
import { useI18n } from "vue-i18n";
import Tooltip from "./Tooltip.vue";

defineProps<{
  label: string;
  forId?: string;
  tooltip?: string;
  asLabel?: boolean;
}>();

const { t } = useI18n();
</script>
