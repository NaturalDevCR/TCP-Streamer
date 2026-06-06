<template>
  <ToastNotification />

  <div class="w-full h-screen flex flex-col overflow-hidden bg-base">
    <!-- Fixed Top Header -->
    <div class="flex flex-col gap-4 pt-4 px-6 shrink-0 z-20 bg-base">
      <AppHeader />
      <StatsBar />
      <TabNav v-model="activeTab" />
    </div>

    <div
      ref="scrollContainer"
      class="flex-1 overflow-y-auto overflow-x-hidden relative hide-scrollbar mt-4"
    >
      <div class="flex flex-col gap-4 min-h-full px-6 pb-6">
        <div class="flex-1 min-h-0">
          <ConnectionTab v-if="activeTab === 'connection'" />
          <AudioTab v-if="activeTab === 'audio'" />
          <SettingsTab v-if="activeTab === 'settings'" />
          <AdvancedTab v-if="activeTab === 'advanced'" />
          <LogsTab v-if="activeTab === 'logs'" />
        </div>

        <AppFooter class="mt-auto pt-4" />
      </div>
    </div>

    <!-- Fixed Bottom Action Bar -->
    <div class="w-full bg-base/90 backdrop-blur-md px-6 py-4 z-30 border-t border-white/5 shrink-0">
      <StreamButton />
    </div>

    <!-- Scroll to Top -->
    <button
      v-if="showScrollTop"
      class="absolute bottom-24 right-6 w-10 h-10 rounded-full bg-accent/80 text-white border-0 flex items-center justify-center cursor-pointer shadow-lg hover:bg-accent transition-all z-40"
      title="Back to top"
      @click="scrollToTop"
    >
      <svg class="w-5 h-5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
        <polyline points="18 15 12 9 6 15" />
      </svg>
    </button>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from "vue";
import { useSettingsStore } from "./stores/settings.ts";
import { useStreamStore } from "./stores/stream.ts";

import ToastNotification from "./components/ToastNotification.vue";
import AppHeader from "./components/AppHeader.vue";
import StatsBar from "./components/StatsBar.vue";
import TabNav from "./components/TabNav.vue";
import StreamButton from "./components/StreamButton.vue";
import AppFooter from "./components/AppFooter.vue";
import ConnectionTab from "./components/tabs/ConnectionTab.vue";
import AudioTab from "./components/tabs/AudioTab.vue";
import SettingsTab from "./components/tabs/SettingsTab.vue";
import AdvancedTab from "./components/tabs/AdvancedTab.vue";
import LogsTab from "./components/tabs/LogsTab.vue";

const settings = useSettingsStore();
const stream = useStreamStore();

const _activeTab = ref("connection");
const activeTab = computed({
  get: () => _activeTab.value,
  set: (val) => {
    if (!document.startViewTransition) {
      _activeTab.value = val;
      return;
    }
    document.startViewTransition(() => {
      _activeTab.value = val;
    });
  },
});

const scrollContainer = ref<HTMLElement | null>(null);
const showScrollTop = ref(false);

onMounted(async () => {
  // Initialize settings and device list
  await settings.init();

  // Initialize event listeners
  await stream.initListeners();

  // Auto-stream on load
  if (settings.autoStream && !stream.isStreaming) {
    setTimeout(() => stream.startStream(), 500);
  }

  // Scroll-to-top visibility
  if (scrollContainer.value) {
    scrollContainer.value.addEventListener("scroll", onScroll);
  }

  // Keyboard shortcut: Ctrl/Cmd+S to toggle streaming
  document.addEventListener("keydown", onKeyDown);
});

onUnmounted(() => {
  if (scrollContainer.value) {
    scrollContainer.value.removeEventListener("scroll", onScroll);
  }
  document.removeEventListener("keydown", onKeyDown);
});

function onKeyDown(e: KeyboardEvent) {
  if ((e.ctrlKey || e.metaKey) && e.key === "s") {
    e.preventDefault();
    stream.toggleStream();
  }
}

function onScroll() {
  showScrollTop.value = Boolean(scrollContainer.value && scrollContainer.value.scrollTop > 200);
}

function scrollToTop() {
  scrollContainer.value?.scrollTo({ top: 0, behavior: "smooth" });
}

// Cleanup on window close
window.addEventListener("beforeunload", (e) => {
  if (stream.isStreaming) {
    e.preventDefault();
    e.returnValue = "Streaming is active. Are you sure you want to close?";
  }
});

// Stop stream on page unload
window.addEventListener("unload", () => {
  if (stream.isStreaming) {
    stream.stopStream();
  }
});
</script>
