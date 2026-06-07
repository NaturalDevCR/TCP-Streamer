<template>
  <DialogRoot :open="open" @update:open="$emit('update:open', $event)">
    <DialogTrigger v-if="$slots.trigger" as-child>
      <slot name="trigger" />
    </DialogTrigger>
    <DialogPortal>
      <DialogOverlay class="fixed inset-0 z-40 bg-black/60" />
      <DialogContent
        class="fixed left-1/2 top-1/2 z-50 w-full max-w-md -translate-x-1/2 -translate-y-1/2 rounded-[var(--radius)] bg-[var(--color-surface-1)] border border-[var(--color-border)] shadow-xl p-6 focus:outline-none"
      >
        <DialogTitle v-if="title" class="text-base font-semibold text-[var(--color-text)] mb-2">
          {{ title }}
        </DialogTitle>
        <DialogDescription v-if="description" class="text-sm text-[var(--color-text-muted)] mb-4">
          {{ description }}
        </DialogDescription>
        <slot />
        <DialogClose
          class="absolute right-4 top-4 rounded-sm text-[var(--color-text-muted)] hover:text-[var(--color-text)] focus-ring"
        >
          <svg
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <line x1="18" y1="6" x2="6" y2="18" />
            <line x1="6" y1="6" x2="18" y2="18" />
          </svg>
        </DialogClose>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>
<script setup lang="ts">
import {
  DialogRoot,
  DialogTrigger,
  DialogPortal,
  DialogOverlay,
  DialogContent,
  DialogTitle,
  DialogDescription,
  DialogClose,
} from "reka-ui";
defineProps<{ open?: boolean; title?: string; description?: string }>();
defineEmits<{ "update:open": [boolean] }>();
</script>
