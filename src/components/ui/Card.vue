<template>
  <section class="panel">
    <header v-if="title || $slots.header" class="panel-header">
      <div class="panel-heading">
        <span v-if="step" class="panel-step">{{ step }}</span>
        <div>
          <h2>{{ title }}</h2>
          <p v-if="description">{{ description }}</p>
        </div>
      </div>
      <slot name="header" />
    </header>
    <div class="panel-body"><slot /></div>
  </section>
</template>
<script setup lang="ts">
defineProps<{ title?: string; description?: string; step?: string }>();
</script>
