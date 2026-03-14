<template>
  <div class="flex flex-col gap-1.5 w-full">
    <label v-if="label" :for="id" class="text-xs font-medium text-slate-400 ml-0.5">{{ label }}</label>
    <div class="relative">
      <select
        :id="id"
        :value="modelValue"
        :disabled="disabled"
        @change="$emit('update:modelValue', $event.target.value)"
        class="input-smooth w-full px-3 py-2.5 bg-black/20 border border-white/10 rounded-lg text-slate-50 text-sm appearance-none cursor-pointer focus:outline-none focus:border-accent focus:ring-2 focus:ring-accent/30 focus:bg-black/40 disabled:opacity-50 disabled:cursor-not-allowed hover:bg-black/30 hover:border-white/20"
      >
        <slot />
      </select>
      <div class="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2">
        <svg class="w-3.5 h-3.5 text-slate-400" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24"><path d="m6 9 6 6 6-6"/></svg>
      </div>
    </div>
  </div>
</template>

<script setup>
defineProps({
  modelValue: [String, Number],
  label: String,
  id: String,
  disabled: Boolean,
});
defineEmits(["update:modelValue"]);
</script>
