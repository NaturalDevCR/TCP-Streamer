<template>
  <ToastNotification />
  <div class="h-full flex bg-[var(--color-bg)]">
    <Sidebar />
    <div class="flex-1 flex flex-col min-w-0">
      <Header />
      <main class="flex-1 overflow-y-auto p-4">
        <DashboardSection v-if="ui.section === 'dashboard'" />
        <ConnectionSection v-if="ui.section === 'connection'" />
        <AudioSection v-if="ui.section === 'audio'" />
        <LogsSection v-if="ui.section === 'logs'" />
        <SettingsSection v-if="ui.section === 'settings'" />
      </main>
    </div>
  </div>
</template>
<script setup lang="ts">
import { useUiStore } from "../stores/ui";
import ToastNotification from "./ToastNotification.vue";
import Sidebar from "./Sidebar.vue";
import Header from "./Header.vue";
import DashboardSection from "./sections/DashboardSection.vue";
import ConnectionSection from "./sections/ConnectionSection.vue";
import AudioSection from "./sections/AudioSection.vue";
import LogsSection from "./sections/LogsSection.vue";
import SettingsSection from "./sections/SettingsSection.vue";
const ui = useUiStore();
</script>
