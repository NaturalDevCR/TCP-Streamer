<template>
  <div class="stat-card">
    <div class="stat-label">
      <span>{{ label }}</span
      ><span v-if="$slots.icon" class="stat-icon" aria-hidden="true"><slot name="icon" /></span>
    </div>
    <span class="stat-value"
      ><slot>{{ value }}</slot></span
    >
  </div>
</template>
<script setup lang="ts">
defineProps<{ label?: string; value?: string | number }>();
</script>
