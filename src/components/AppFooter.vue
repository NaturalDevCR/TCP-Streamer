<template>
  <div class="flex flex-col items-center gap-3 py-4 text-xs text-slate-400">
    <div class="flex items-center gap-2">
      Made with ♥ by Josue O.A / NaturalDevCR
      <span class="opacity-50">v{{ version }}</span>
    </div>
    <a
      href="https://paypal.me/NaturalCloud"
      target="_blank"
      class="inline-flex items-center gap-1.5 px-3 py-1.5 bg-gradient-to-r from-[#0070ba] to-[#1546a0] text-white no-underline rounded-md text-xs font-semibold shadow-md hover:-translate-y-0.5 hover:shadow-lg transition-all"
    >
      <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><path d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z"/></svg>
      Donate via PayPal
    </a>
  </div>
</template>

<script setup>
const version = import.meta.env.PACKAGE_VERSION || "?.?.?";
</script>
