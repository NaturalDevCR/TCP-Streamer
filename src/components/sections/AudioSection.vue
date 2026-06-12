<template>
  <div class="flex flex-col gap-4">
    <Card :title="t('audio.title')">
      <div class="flex flex-col gap-4">
        <div class="flex gap-3">
          <Field
            v-if="settings.role === 'source'"
            :label="t('audio.inputDevice')"
            :tooltip="t('audio.help.inputDevice')"
            class="flex-1"
          >
            <Select
              v-model="settings.deviceName"
              :disabled="stream.isStreaming || settings.devicesLoading"
            >
              <SelectOption
                v-if="settings.devicesLoading"
                value=""
                :label="t('connection.loadingDevices')"
              />
              <SelectOption
                v-else-if="settings.devices.length === 0"
                value=""
                :label="t('connection.noDevices')"
              />
              <SelectOption v-for="d in settings.devices" :key="d" :value="d" :label="d" />
            </Select>
          </Field>

          <Field
            :label="t('audio.sampleRate')"
            :tooltip="t('audio.help.sampleRate')"
            class="w-[140px]"
          >
            <Select v-model.number="settings.sampleRate" :disabled="stream.isStreaming">
              <SelectOption :value="48000" label="48 kHz" />
              <SelectOption :value="44100" label="44.1 kHz" />
            </Select>
          </Field>
        </div>

        <div class="flex gap-3">
          <Field
            :label="t('audio.bufferSize')"
            :tooltip="t('audio.help.bufferSize')"
            class="flex-1"
          >
            <Select v-model.number="settings.bufferSize" :disabled="stream.isStreaming">
              <SelectOption :value="256" label="256 (Low)" />
              <SelectOption :value="512" label="512 (Medium)" />
              <SelectOption :value="1024" label="1024 (High)" />
              <SelectOption :value="2048" label="2048 (Max)" />
            </Select>
          </Field>

          <Field
            v-if="settings.isServer"
            :label="t('audio.format')"
            :tooltip="t('audio.help.format')"
            class="flex-1"
          >
            <Select v-model="settings.format" :disabled="stream.isStreaming">
              <SelectOption value="pcm" :label="t('audio.formatPcm')" />
              <SelectOption value="wav" :label="t('audio.formatWav')" />
            </Select>
          </Field>
        </div>
      </div>
    </Card>

    <Card>
      <div class="flex flex-col gap-4">
        <template v-if="settings.showLoopback">
          <div class="flex items-center justify-between">
            <SettingLabel :label="t('audio.loopback')" :tooltip="t('audio.help.loopback')" />
            <Switch v-model="settings.loopbackMode" @update:model-value="onLoopbackChange" />
          </div>
        </template>

        <Field :label="t('audio.latencyProfile')" :tooltip="t('audio.help.latencyProfile')">
          <Select v-model="settings.latencyProfile" :disabled="stream.isStreaming">
            <SelectOption value="ultra-low" :label="t('audio.latencyUltraLow')" />
            <SelectOption value="balanced" :label="t('audio.latencyBalanced')" />
            <SelectOption value="robust" :label="t('audio.latencyRobust')" />
            <SelectOption value="custom" :label="t('audio.latencyCustom')" />
          </Select>
        </Field>

        <p
          v-if="settings.latencyProfile === 'ultra-low' && settings.isLoopback"
          class="text-xs text-[var(--color-warning)]"
        >
          {{ t("audio.ultraLowWarning") }}
        </p>

        <template v-if="settings.latencyProfile === 'custom'">
          <div class="flex gap-3">
            <Field
              :label="t('audio.ringBufferDuration')"
              :tooltip="t('audio.help.ringBufferDuration')"
              class="flex-1"
            >
              <Select v-model.number="settings.ringBufferDuration" :disabled="stream.isStreaming">
                <SelectOption :value="2000" label="2000ms (Low)" />
                <SelectOption :value="4000" label="4000ms (Balanced)" />
                <SelectOption :value="8000" label="8000ms (High)" />
                <SelectOption :value="15000" label="15000ms (Max)" />
              </Select>
            </Field>
            <Field
              :label="t('audio.chunkSize')"
              :tooltip="t('audio.help.chunkSize')"
              class="flex-1"
            >
              <Select v-model.number="settings.chunkSize" :disabled="stream.isStreaming">
                <SelectOption :value="128" label="128" />
                <SelectOption :value="256" label="256" />
                <SelectOption :value="512" label="512" />
                <SelectOption :value="1024" label="1024" />
                <SelectOption :value="2048" label="2048" />
                <SelectOption :value="4096" label="4096" />
              </Select>
            </Field>
          </div>

          <div class="flex items-center justify-between">
            <SettingLabel
              :label="t('audio.adaptiveBuffer')"
              :tooltip="t('audio.help.adaptiveBuffer')"
            />
            <Switch v-model="settings.adaptiveBuffer" />
          </div>

          <div class="flex gap-3">
            <Field
              :label="t('audio.minBuffer')"
              :tooltip="t('audio.help.minBuffer')"
              class="flex-1"
            >
              <Input
                v-model.number="settings.minBuffer"
                type="number"
                :min="1000"
                :max="5000"
                :step="500"
              />
            </Field>
            <Field
              :label="t('audio.maxBuffer')"
              :tooltip="t('audio.help.maxBuffer')"
              class="flex-1"
            >
              <Input
                v-model.number="settings.maxBuffer"
                type="number"
                :min="5000"
                :max="20000"
                :step="1000"
              />
            </Field>
          </div>
        </template>
      </div>
    </Card>
  </div>
</template>
<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settings";
import { useStreamStore } from "../../stores/stream";
import Card from "../ui/Card.vue";
import Field from "../ui/Field.vue";
import Input from "../ui/Input.vue";
import Select from "../ui/Select.vue";
import SelectOption from "../ui/SelectOption.vue";
import Switch from "../ui/Switch.vue";
import SettingLabel from "../ui/SettingLabel.vue";

const { t } = useI18n();
const settings = useSettingsStore();
const stream = useStreamStore();

async function onLoopbackChange() {
  await settings.loadDevices();
}
</script>
