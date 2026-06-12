<template>
  <div class="flex flex-col gap-4">
    <Card>
      <Field :label="t('connection.role')" :tooltip="t('connection.help.role')">
        <SegmentedControl
          v-model="settings.role"
          :options="roleOptions"
          :disabled="stream.isStreaming"
        />
      </Field>
    </Card>

    <!-- SOURCE -->
    <template v-if="settings.role === 'source'">
      <Card :title="t('nav.connection')">
        <div class="flex flex-col gap-4">
          <Field :label="t('audio.inputDevice')" :tooltip="t('connection.help.inputDevice')">
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

          <p v-if="settings.devicesError" class="text-xs text-[var(--color-danger)]">
            {{ settings.devicesError }}
          </p>

          <template v-if="settings.showLoopback">
            <div class="flex items-center justify-between">
              <SettingLabel :label="t('audio.loopback')" :tooltip="t('connection.help.loopback')" />
              <Switch v-model="settings.loopbackMode" @update:model-value="onLoopbackChange" />
            </div>
          </template>

          <Field :label="t('connection.transport')" :tooltip="t('connection.help.transport')">
            <Select v-model="settings.transport" :disabled="stream.isStreaming">
              <SelectOption value="tcp" :label="t('connection.transportTcp')" />
              <SelectOption value="udp" :label="t('connection.transportUdp')" />
            </Select>
          </Field>

          <Field
            v-if="settings.transport === 'tcp'"
            :label="t('connection.mode')"
            :tooltip="t('connection.help.mode')"
          >
            <Select v-model="settings.mode" :disabled="stream.isStreaming">
              <SelectOption value="client" :label="t('connection.modeClient')" />
              <SelectOption value="server" :label="t('connection.modeServer')" />
            </Select>
          </Field>

          <Field
            v-if="settings.transport === 'udp'"
            :label="t('connection.psk')"
            :tooltip="t('connection.help.psk')"
          >
            <Input
              v-model="settings.psk"
              type="password"
              :placeholder="t('connection.pskPlaceholder')"
              :disabled="stream.isStreaming"
            />
          </Field>
        </div>
      </Card>

      <!-- TCP Destination -->
      <Card v-if="settings.transport === 'tcp'">
        <div class="flex flex-col gap-4">
          <div class="flex gap-3">
            <Field
              v-if="!settings.isServer"
              :label="t('connection.targetAddress')"
              :tooltip="t('connection.help.targetAddress')"
              class="flex-1"
            >
              <Input
                v-model="settings.ip"
                :placeholder="t('connection.targetAddressPlaceholder')"
                :disabled="stream.isStreaming"
              />
            </Field>
            <Field
              :label="settings.isServer ? t('connection.listeningPort') : t('connection.port')"
              :tooltip="t('connection.help.port')"
              :class="settings.isServer ? 'flex-1' : 'w-[120px]'"
            >
              <Input v-model.number="settings.port" type="number" :disabled="stream.isStreaming" />
            </Field>
          </div>

          <Field
            v-if="settings.isServer"
            :label="t('connection.allowlist')"
            :tooltip="t('connection.help.allowlist')"
          >
            <Input
              v-model="settings.allowlist"
              placeholder="192.168.1.0/24, 10.0.0.5"
              :disabled="stream.isStreaming"
            />
          </Field>

          <Card v-if="settings.isServer && stream.isStreaming" :title="t('connection.info')">
            <div class="flex flex-col gap-3">
              <CopyField v-model="stream.tcpUrl" label="TCP" />
              <CopyField v-model="stream.httpUrl" label="HTTP" />
              <Textarea v-model="stream.connectionInfo" :rows="3" readonly />
            </div>
          </Card>
        </div>
      </Card>

      <!-- Native UDP -->
      <Card v-if="settings.transport === 'udp'">
        <div class="flex flex-col gap-4">
          <Field :label="t('connection.port')" :tooltip="t('connection.help.port')">
            <Input v-model.number="settings.port" type="number" :disabled="stream.isStreaming" />
          </Field>
        </div>
      </Card>
    </template>

    <!-- SINK -->
    <template v-if="settings.role === 'sink'">
      <Card :title="t('connection.roleSink')">
        <div class="flex flex-col gap-4">
          <Field :label="t('connection.outputDevice')" :tooltip="t('connection.help.outputDevice')">
            <Select v-model="settings.outputDevice" :disabled="stream.isStreaming">
              <SelectOption
                v-if="settings.outputDevices.length === 0"
                value=""
                :label="t('connection.noOutputDevices')"
              />
              <SelectOption v-for="d in settings.outputDevices" :key="d" :value="d" :label="d" />
            </Select>
          </Field>

          <Field
            :label="t('connection.sourceAddress')"
            :tooltip="t('connection.help.sourceAddress')"
          >
            <Input
              v-model="settings.sourceAddr"
              :placeholder="t('connection.sourceAddressPlaceholder')"
              :disabled="stream.isStreaming"
            />
          </Field>

          <div class="flex gap-2 items-end">
            <Button
              size="sm"
              variant="secondary"
              :disabled="stream.isStreaming"
              @click="settings.scanSources()"
            >
              {{ t("action.scan") }}
            </Button>
            <span
              v-if="settings.discoveredSources.length"
              class="text-xs text-[var(--color-text-muted)] self-center"
            >
              {{ settings.discoveredSources.length }} found
            </span>
          </div>

          <Field
            v-if="settings.discoveredSources.length"
            :label="t('connection.discoveredSources')"
            :tooltip="t('connection.help.discoveredSources')"
          >
            <Select
              :model-value="settings.sourceAddr"
              :disabled="stream.isStreaming"
              @update:model-value="(v: string | number) => (settings.sourceAddr = String(v))"
            >
              <SelectOption value="" :label="t('connection.chooseSource')" />
              <SelectOption
                v-for="s in settings.discoveredSources"
                :key="s.addr"
                :value="s.addr"
                :label="`${s.name} (${s.addr})${s.encrypted ? ' [encrypted]' : ''}`"
              />
            </Select>
          </Field>

          <Field :label="t('connection.psk')" :tooltip="t('connection.help.psk')">
            <Input
              v-model="settings.psk"
              type="password"
              :placeholder="t('connection.pskPlaceholder')"
              :disabled="stream.isStreaming"
            />
          </Field>
        </div>
      </Card>
    </template>
  </div>
</template>
<script setup lang="ts">
import { computed, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settings";
import { useStreamStore } from "../../stores/stream";
import Card from "../ui/Card.vue";
import Field from "../ui/Field.vue";
import Input from "../ui/Input.vue";
import Textarea from "../ui/Textarea.vue";
import Select from "../ui/Select.vue";
import SelectOption from "../ui/SelectOption.vue";
import Switch from "../ui/Switch.vue";
import Button from "../ui/Button.vue";
import SegmentedControl from "../ui/SegmentedControl.vue";
import CopyField from "../ui/CopyField.vue";
import SettingLabel from "../ui/SettingLabel.vue";

const { t } = useI18n();
const settings = useSettingsStore();
const stream = useStreamStore();

const roleOptions = computed(() => [
  { value: "source", label: t("connection.roleSource") },
  { value: "sink", label: t("connection.roleSink") },
]);

onMounted(async () => {
  await settings.loadOutputDevices();
});

async function onLoopbackChange() {
  await settings.loadDevices();
}
</script>
