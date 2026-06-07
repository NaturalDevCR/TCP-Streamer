<template>
  <div class="flex flex-col gap-1.5 w-full">
    <label v-if="label" class="text-xs font-medium text-[var(--color-text-muted)] ml-0.5">
      {{ label }}
    </label>
    <div class="flex gap-2 items-stretch">
      <input
        ref="inputRef"
        :value="modelValue"
        readonly
        :class="inputClass"
        class="flex-1 px-3 py-2 bg-[var(--color-surface-1)] border border-[var(--color-border)] rounded-[var(--radius)] text-sm font-mono"
      />
      <Button size="sm" variant="ghost" @click="copy">
        <IconCopy />
      </Button>
    </div>
  </div>
</template>
<script setup lang="ts">
import { ref } from "vue";
import { useStreamStore } from "../../stores/stream";
import { IconCopy } from "../icons";
import Button from "./Button.vue";

const props = defineProps<{
  modelValue?: string;
  label?: string;
  inputClass?: string;
}>();

const inputRef = ref<HTMLInputElement | null>(null);
const stream = useStreamStore();

function copy() {
  if (props.modelValue) {
    navigator.clipboard
      .writeText(props.modelValue)
      .then(() => stream.addToast("Copied to clipboard!", "success"))
      .catch(() => stream.addToast("Failed to copy", "error"));
  }
}
</script>
