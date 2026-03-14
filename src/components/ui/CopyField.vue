<template>
  <div class="flex flex-col gap-1.5 w-full">
    <label v-if="label" class="text-xs font-medium text-slate-400 ml-0.5">{{ label }}</label>
    <div class="flex gap-2 items-stretch">
      <input
        ref="inputRef"
        :value="modelValue"
        readonly
        :class="inputClass"
        class="flex-1 px-3 py-2 bg-black/30 border border-white/10 rounded-lg text-sm font-mono transition-all"
      />
      <button
        @click="copy"
        class="w-9 h-auto flex items-center justify-center bg-white/10 border border-white/10 rounded-lg text-white/80 hover:bg-white/20 transition-all cursor-pointer text-sm"
        title="Copy"
      >
        📋
      </button>
    </div>
  </div>
</template>

<script setup>
import { ref } from "vue";
import { useStreamStore } from "../../stores/stream.js";

const props = defineProps({
  modelValue: String,
  label: String,
  inputClass: { type: String, default: "text-slate-50" },
});

const inputRef = ref(null);
const stream = useStreamStore();

function copy() {
  if (props.modelValue) {
    navigator.clipboard.writeText(props.modelValue)
      .then(() => stream.addToast("Copied to clipboard!", "success"))
      .catch(() => stream.addToast("Failed to copy", "error"));
  }
}
</script>
