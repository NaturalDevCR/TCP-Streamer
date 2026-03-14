<template>
  <LinuxTitlebar />
  <ToastNotification />

  <div
    :class="[
      'w-full h-screen flex flex-col gap-4 overflow-y-auto overflow-x-hidden px-6 py-4 hide-scrollbar',
      settings.osType === 'linux' ? 'pt-12' : ''
    ]"
    ref="scrollContainer"
  >
    <AppHeader />
    <StatsBar />
    <TabNav v-model="activeTab" />

    <div class="flex-1 min-h-0">
      <ConnectionTab v-if="activeTab === 'connection'" />
      <AudioTab v-if="activeTab === 'audio'" />
      <SettingsTab v-if="activeTab === 'settings'" />
      <AdvancedTab v-if="activeTab === 'advanced'" />
      <LogsTab v-if="activeTab === 'logs'" />
    </div>

    <StreamButton />
    <AppFooter />

    <!-- Scroll to Top -->
    <button
      v-if="showScrollTop"
      @click="scrollToTop"
      class="fixed bottom-6 right-6 w-10 h-10 rounded-full bg-accent/80 text-white border-0 flex items-center justify-center cursor-pointer shadow-lg hover:bg-accent transition-all z-40"
      title="Back to top"
    >
      <svg class="w-5 h-5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24"><polyline points="18 15 12 9 6 15"/></svg>
    </button>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted } from "vue";
import { useSettingsStore } from "./stores/settings.js";
import { useStreamStore } from "./stores/stream.js";

import LinuxTitlebar from "./components/LinuxTitlebar.vue";
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
  }
});

const scrollContainer = ref(null);
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
});

onUnmounted(() => {
  if (scrollContainer.value) {
    scrollContainer.value.removeEventListener("scroll", onScroll);
  }
});

function onScroll() {
  showScrollTop.value = scrollContainer.value && scrollContainer.value.scrollTop > 200;
}

function scrollToTop() {
  scrollContainer.value?.scrollTo({ top: 0, behavior: "smooth" });
}

// Cleanup on window close
window.addEventListener("beforeunload", async () => {
  if (stream.isStreaming) {
    await stream.stopStream();
  }
});
</script>
