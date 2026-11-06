<template>
  <fieldset
    :disabled="settings.configurationLocked || settings.initializing || settings.profileBusy"
    class="configuration-grid audio-grid"
  >
    <p
      v-if="settings.role === 'sink'"
      class="span-full notice text-xs text-[var(--color-text-muted)]"
    >
      {{ t("operation.sinkAudio") }}
    </p>
    <Card
      v-if="settings.role === 'source'"
      class="span-full"
      step="01"
      :title="t('workspace.audioFormat')"
      :description="t('workspace.audioFormatDescription')"
    >
      <div class="flex flex-col gap-4">
        <div class="form-row">
          <Field
            v-if="settings.role === 'source'"
            :label="t('audio.inputDevice')"
            :tooltip="t('audio.help.inputDevice')"
            :error="
              settings.inputUnavailable
                ? t('connection.unavailableDevice', { name: settings.deviceName })
                : undefined
            "
            class="flex-1"
          >
            <Select
              :model-value="settings.inputUnavailable ? '' : settings.deviceName"
              :placeholder="
                !settings.nativeAvailable
                  ? t('runtime.desktopDevices')
                  : settings.devicesLoading
                    ? t('connection.loadingDevices')
                    : settings.devices.length
                      ? t('connection.chooseDevice')
                      : t('connection.noDevices')
              "
              :disabled="
                !settings.nativeAvailable || settings.configurationLocked || settings.devicesLoading
              "
              @update:model-value="settings.deviceName = String($event)"
            >
              <SelectOption v-for="d in settings.devices" :key="d" :value="d" :label="d" />
            </Select>
          </Field>

          <Field
            :label="t('audio.sampleRate')"
            :tooltip="t('audio.help.sampleRate')"
            class="w-[180px]"
          >
            <Select v-model.number="settings.sampleRate" :disabled="settings.configurationLocked">
              <SelectOption :value="48000" label="48 kHz" />
              <SelectOption :value="44100" label="44.1 kHz" />
            </Select>
          </Field>
        </div>

        <div class="form-row">
          <Field
            :label="t('audio.bufferSize')"
            :tooltip="t('audio.help.bufferSize')"
            class="flex-1"
          >
            <Select v-model.number="settings.bufferSize" :disabled="settings.configurationLocked">
              <SelectOption :value="256" label="256" />
              <SelectOption :value="512" label="512" />
              <SelectOption :value="1024" label="1024" />
              <SelectOption :value="2048" label="2048" />
            </Select>
          </Field>

          <Field
            v-if="settings.isServer && settings.transport === 'tcp'"
            :label="t('audio.format')"
            :tooltip="t('audio.help.format')"
            class="flex-1"
          >
            <Select v-model="settings.format" :disabled="settings.configurationLocked">
              <SelectOption value="pcm" :label="t('audio.formatPcm')" />
              <SelectOption value="wav" :label="t('audio.formatWav')" />
            </Select>
          </Field>
        </div>
      </div>
    </Card>

    <Card
      class="span-full"
      step="02"
      :title="t('workspace.bufferAndLatency')"
      :description="t('workspace.bufferDescription')"
    >
      <div class="flex flex-col gap-4">
        <template v-if="settings.role === 'source' && settings.showLoopback">
          <div class="flex items-center justify-between">
            <SettingLabel :label="t('audio.loopback')" :tooltip="t('audio.help.loopback')" />
            <Switch
              v-model="settings.loopbackMode"
              :aria-label="t('audio.loopback')"
              @update:model-value="onLoopbackChange"
            />
          </div>
        </template>

        <Field :label="t('audio.latencyProfile')" :tooltip="t('audio.help.latencyProfile')">
          <SegmentedControl
            v-model="settings.latencyProfile"
            class="latency-presets"
            :options="latencyOptions"
            :disabled="settings.configurationLocked"
          />
        </Field>

        <template v-if="settings.latencyProfile === 'broadcast'">
          <Field :label="t('audio.fixedLatency')" :tooltip="t('audio.help.fixedLatency')">
            <Input
              v-model.number="settings.fixedLatencyMs"
              type="number"
              :min="50"
              :max="2000"
              :step="50"
              :disabled="settings.configurationLocked"
            />
          </Field>
          <p class="text-xs text-[var(--color-text-muted)]">
            {{ t("audio.broadcastNote") }}
          </p>
        </template>

        <p
          v-if="settings.latencyProfile === 'ultra-low' && settings.isLoopback"
          class="text-xs text-[var(--color-warning)]"
        >
          {{ t("audio.ultraLowWarning") }}
        </p>

        <div
          v-if="settings.role === 'source' && settings.latencyProfile !== 'broadcast'"
          class="flex items-center justify-between"
        >
          <SettingLabel
            :label="t('audio.adaptiveBuffer')"
            :tooltip="t('audio.help.adaptiveBuffer')"
          />
          <Switch v-model="settings.adaptiveBuffer" :aria-label="t('audio.adaptiveBuffer')" />
        </div>
        <template v-if="settings.latencyProfile === 'custom'">
          <div class="form-row">
            <Field
              :label="t('audio.ringBufferDuration')"
              :tooltip="t('audio.help.ringBufferDuration')"
              class="flex-1"
            >
              <Select
                v-model.number="settings.ringBufferDuration"
                :disabled="settings.configurationLocked"
              >
                <SelectOption :value="2000" label="2000ms" />
                <SelectOption :value="4000" label="4000ms" />
                <SelectOption :value="8000" label="8000ms" />
                <SelectOption :value="15000" label="15000ms" />
              </Select>
            </Field>
            <Field
              v-if="settings.role === 'source'"
              :label="t('audio.chunkSize')"
              :tooltip="t('audio.help.chunkSize')"
              class="flex-1"
            >
              <Select v-model.number="settings.chunkSize" :disabled="settings.configurationLocked">
                <SelectOption :value="128" label="128" />
                <SelectOption :value="256" label="256" />
                <SelectOption :value="512" label="512" />
                <SelectOption :value="1024" label="1024" />
                <SelectOption :value="2048" label="2048" />
                <SelectOption :value="4096" label="4096" />
              </Select>
            </Field>
          </div>

          <div class="form-row">
            <Field
              :label="t('audio.minBuffer')"
              :tooltip="t('audio.help.minBuffer')"
              class="flex-1"
            >
              <Input
                v-model.number="settings.minBuffer"
                type="number"
                :min="50"
                :max="20000"
                :step="500"
              />
            </Field>
            <Field
              :error="
                settings.maxBuffer > settings.ringBufferDuration ||
                settings.minBuffer > settings.maxBuffer
                  ? t('operation.invalidBuffer')
                  : undefined
              "
              :label="t('audio.maxBuffer')"
              :tooltip="t('audio.help.maxBuffer')"
              class="flex-1"
            >
              <Input
                v-model.number="settings.maxBuffer"
                type="number"
                :min="50"
                :max="20000"
                :step="1000"
              />
            </Field>
          </div>
        </template>
      </div>
    </Card>
  </fieldset>
</template>
<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settings";
import SegmentedControl from "../ui/SegmentedControl.vue";
import Card from "../ui/Card.vue";
import Field from "../ui/Field.vue";
import Input from "../ui/Input.vue";
import Select from "../ui/Select.vue";
import SelectOption from "../ui/SelectOption.vue";
import Switch from "../ui/Switch.vue";
import SettingLabel from "../ui/SettingLabel.vue";

const { t } = useI18n();
const settings = useSettingsStore();
const latencyOptions = computed(() => [
  {
    value: "ultra-low",
    label: t("audio.latencyUltraLow"),
    description: t("workspace.presets.ultraLow"),
  },
  {
    value: "balanced",
    label: t("audio.latencyBalanced"),
    description: t("workspace.presets.balanced"),
  },
  { value: "robust", label: t("audio.latencyRobust"), description: t("workspace.presets.robust") },
  {
    value: "broadcast",
    label: t("audio.latencyBroadcast"),
    description: t("workspace.presets.broadcast"),
  },
  { value: "custom", label: t("audio.latencyCustom"), description: t("workspace.presets.custom") },
]);

async function onLoopbackChange() {
  await settings.loadDevices();
}
</script>
