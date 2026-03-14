<template>
  <div class="flex flex-col gap-1.5 w-full">
    <label v-if="label" :for="id" class="text-xs font-medium text-slate-400 ml-0.5">{{ label }}</label>
    <input
      :id="id"
      :type="type"
      :value="modelValue"
      :placeholder="placeholder"
      :disabled="disabled"
      :readonly="readonly"
      :min="min"
      :max="max"
      :step="step"
      @input="$emit('update:modelValue', type === 'number' ? Number($event.target.value) : $event.target.value)"
      class="input-smooth w-full px-3 py-2.5 bg-black/20 border border-white/10 rounded-lg text-slate-50 text-sm focus:outline-none focus:border-accent focus:ring-2 focus:ring-accent/30 focus:bg-black/40 disabled:opacity-50 disabled:cursor-not-allowed hover:bg-black/30 hover:border-white/20"
    />
  </div>
</template>

<script setup>
defineProps({
  modelValue: [String, Number],
  label: String,
  id: String,
  type: { type: String, default: "text" },
  placeholder: String,
  disabled: Boolean,
  readonly: Boolean,
  min: [String, Number],
  max: [String, Number],
  step: [String, Number],
});
defineEmits(["update:modelValue"]);
</script>
