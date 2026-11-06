<template>
  <header class="workspace-header">
    <div class="breadcrumb">
      <span>{{ t("workspace.workspace") }}</span
      ><IconChevronRight /><span>{{ t(`nav.${ui.section}`) }}</span>
    </div>
    <div class="header-actions">
      <Select
        class="header-locale"
        :aria-label="t('settings.language')"
        :model-value="ui.locale"
        @update:model-value="(v) => ui.setLocale(v as 'en' | 'es')"
      >
        <SelectOption value="en" label="English" /><SelectOption value="es" label="Español" />
      </Select>
      <span class="header-status" role="status"
        ><StatusDot :tone="stream.statusTone" :label="stream.statusText"
      /></span>
      <Button
        v-if="settings.isDirty"
        size="sm"
        variant="secondary"
        :disabled="
          !settings.nativeAvailable ||
          settings.configurationLocked ||
          settings.initializing ||
          settings.profileBusy
        "
        @click="stream.saveConfiguration()"
        ><IconSave class="w-3.5 h-3.5" />{{ t("operation.save") }}</Button
      >
      <Button
        :variant="stream.pending || stream.isStreaming ? 'danger' : 'primary'"
        size="sm"
        :disabled="
          !settings.nativeAvailable ||
          settings.initializing ||
          stream.stopping ||
          settings.preflightLoading ||
          (!stream.pending &&
            !stream.isStreaming &&
            (settings.profileBusy ||
              (settings.role === 'sink' ? settings.outputDevicesLoading : settings.devicesLoading)))
        "
        :title="t('operation.shortcuts')"
        @click="stream.toggleStream()"
      >
        <IconPlay v-if="!stream.pending && !stream.isStreaming" class="w-3.5 h-3.5" />
        <IconStop v-else class="w-3.5 h-3.5" />
        {{ stream.pending || stream.isStreaming ? t("action.stop") : t("workspace.startStream") }}
      </Button>
    </div>
  </header>
</template>
<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { useStreamStore } from "../stores/stream";
import { useSettingsStore } from "../stores/settings";
import { useUiStore } from "../stores/ui";
import Button from "./ui/Button.vue";
import Select from "./ui/Select.vue";
import SelectOption from "./ui/SelectOption.vue";
import StatusDot from "./ui/StatusDot.vue";
import { IconPlay, IconStop, IconChevronRight, IconSave } from "./icons";
const { t } = useI18n();
const stream = useStreamStore();
const ui = useUiStore();
const settings = useSettingsStore();
</script>
