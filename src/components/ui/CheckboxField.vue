<template>
  <label class="flex items-center gap-2.5 cursor-pointer select-none group">
    <div class="relative">
      <input
        type="checkbox"
        :checked="modelValue"
        :disabled="disabled"
        @change="$emit('update:modelValue', $event.target.checked)"
        class="peer sr-only"
      />
      <div class="w-[18px] h-[18px] rounded border border-white/30 bg-black/20 peer-checked:bg-accent peer-checked:border-accent transition-all flex items-center justify-center peer-disabled:opacity-50">
        <svg v-if="modelValue" class="w-3 h-3 text-white" fill="none" stroke="currentColor" stroke-width="3" viewBox="0 0 24 24"><path d="M5 13l4 4L19 7"/></svg>
      </div>
    </div>
    <span class="text-sm text-slate-200 group-hover:text-white transition-colors">{{ label }}</span>
  </label>
  <p v-if="description" class="text-[11px] text-white/50 ml-7 mt-1 leading-tight">{{ description }}</p>
</template>

<script setup>
defineProps({
  modelValue: Boolean,
  label: String,
  description: String,
  disabled: Boolean,
});
defineEmits(["update:modelValue"]);
</script>
