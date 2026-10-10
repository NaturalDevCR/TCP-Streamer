<template>
  <fieldset :disabled="settings.configurationLocked" class="flex flex-col gap-4 min-w-0">
    <p v-if="settings.role === 'sink'" class="text-xs text-[var(--color-text-muted)]">
      {{ t("operation.sinkAudio") }}
    </p>
    <Card v-if="settings.role === 'source'" :title="t('audio.title')">
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
              :placeholder="
                settings.devicesLoading ? t('connection.loadingDevices') : t('connection.noDevices')
              "
              :disabled="settings.configurationLocked || settings.devicesLoading"
            >
              <SelectOption v-for="d in settings.devices" :key="d" :value="d" :label="d" />
            </Select>
          </Field>

          <Field
            :label="t('audio.sampleRate')"
            :tooltip="t('audio.help.sampleRate')"
            class="w-[140px]"
          >
            <Select v-model.number="settings.sampleRate" :disabled="settings.configurationLocked">
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

    <Card>
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
          <Select v-model="settings.latencyProfile" :disabled="settings.configurationLocked">
            <SelectOption value="ultra-low" :label="t('audio.latencyUltraLow')" />
            <SelectOption value="balanced" :label="t('audio.latencyBalanced')" />
            <SelectOption value="robust" :label="t('audio.latencyRobust')" />
            <SelectOption value="broadcast" :label="t('audio.latencyBroadcast')" />
            <SelectOption value="custom" :label="t('audio.latencyCustom')" />
          </Select>
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
          <div class="flex gap-3">
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

          <div class="flex gap-3">
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
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settings";
import Card from "../ui/Card.vue";
import Field from "../ui/Field.vue";
import Input from "../ui/Input.vue";
import Select from "../ui/Select.vue";
import SelectOption from "../ui/SelectOption.vue";
import Switch from "../ui/Switch.vue";
import SettingLabel from "../ui/SettingLabel.vue";

const { t } = useI18n();
const settings = useSettingsStore();

async function onLoopbackChange() {
  await settings.loadDevices();
}
</script>
