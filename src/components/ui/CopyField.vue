<template>
  <div class="flex flex-col gap-1.5 w-full">
    <label v-if="label" :for="id" class="text-xs font-medium text-[var(--color-text-muted)] ml-0.5">
      {{ label }}
    </label>
    <div class="flex gap-2 items-stretch">
      <input
        :id="id"
        :value="modelValue"
        readonly
        :class="inputClass"
        class="focus-ring min-w-0 flex-1 px-3 py-2 bg-[var(--color-surface-1)] border border-[var(--color-border)] rounded-[var(--radius)] text-sm font-mono"
      />
      <Button :aria-label="t('action.copy')" size="sm" variant="ghost" @click="copy">
        <IconCopy />
      </Button>
    </div>
  </div>
</template>
<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { useId } from "vue";
import { useStreamStore } from "../../stores/stream";
import { IconCopy } from "../icons";
import Button from "./Button.vue";

const props = defineProps<{
  modelValue?: string;
  label?: string;
  inputClass?: string;
}>();

const id = useId();
const { t } = useI18n();
const stream = useStreamStore();

async function copy() {
  if (!props.modelValue) return;
  try {
    await navigator.clipboard.writeText(props.modelValue);
    stream.addToast(t("toast.copied"), "success");
  } catch {
    stream.addToast(t("toast.copyFailed"), "error");
  }
}
</script>
