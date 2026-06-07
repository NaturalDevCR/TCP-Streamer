<template>
  <TooltipProvider :delay-duration="200">
    <TooltipRoot>
      <TooltipTrigger as-child>
        <slot name="trigger" />
      </TooltipTrigger>
      <TooltipPortal>
        <TooltipContent
          :side-offset="4"
          class="z-50 rounded-[var(--radius)] bg-[var(--color-surface-3)] border border-[var(--color-border)] px-3 py-1.5 text-xs text-[var(--color-text)] shadow-md"
        >
          <slot />
          <TooltipArrow class="fill-[var(--color-surface-3)]" />
        </TooltipContent>
      </TooltipPortal>
    </TooltipRoot>
  </TooltipProvider>
</template>
<script setup lang="ts">
import {
  TooltipProvider,
  TooltipRoot,
  TooltipTrigger,
  TooltipContent,
  TooltipArrow,
  TooltipPortal,
} from "reka-ui";
</script>
