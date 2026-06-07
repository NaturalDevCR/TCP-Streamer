<template>
  <div class="flex flex-col gap-4">
    <Card>
      <Field :label="t('connection.roleSource')">
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
          <Field :label="t('audio.inputDevice')">
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
              <div>
                <p class="text-sm text-[var(--color-text)]">{{ t("audio.loopback") }}</p>
                <p class="text-xs text-[var(--color-text-faint)]">{{ t("audio.loopbackDesc") }}</p>
              </div>
              <Switch v-model="settings.loopbackMode" @update:model-value="onLoopbackChange" />
            </div>
          </template>

          <Field label="Transport">
            <Select v-model="settings.transport" :disabled="stream.isStreaming">
              <SelectOption value="tcp" :label="t('connection.transportTcp')" />
              <SelectOption value="udp" :label="t('connection.transportUdp')" />
            </Select>
          </Field>

          <Field v-if="settings.transport === 'tcp'" label="Mode">
            <Select v-model="settings.mode" :disabled="stream.isStreaming">
              <SelectOption value="client" :label="t('connection.modeClient')" />
              <SelectOption value="server" :label="t('connection.modeServer')" />
            </Select>
          </Field>

          <Field
            v-if="settings.transport === 'udp'"
            :label="t('connection.psk')"
            :help="t('connection.pskHelp')"
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
            <Field v-if="!settings.isServer" :label="t('connection.targetAddress')" class="flex-1">
              <Input
                v-model="settings.ip"
                :placeholder="t('connection.targetAddressPlaceholder')"
                :disabled="stream.isStreaming"
              />
            </Field>
            <Field
              :label="settings.isServer ? t('connection.listeningPort') : t('connection.port')"
              :class="settings.isServer ? 'flex-1' : 'w-[120px]'"
            >
              <Input v-model.number="settings.port" type="number" :disabled="stream.isStreaming" />
            </Field>
          </div>

          <Field
            v-if="settings.isServer"
            :label="t('connection.allowlist')"
            :help="t('connection.allowlistHelp')"
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
          <Field :label="t('connection.port')">
            <Input v-model.number="settings.port" type="number" :disabled="stream.isStreaming" />
          </Field>
        </div>
      </Card>
    </template>

    <!-- SINK -->
    <template v-if="settings.role === 'sink'">
      <Card :title="t('connection.roleSink')">
        <div class="flex flex-col gap-4">
          <Field :label="t('connection.outputDevice')">
            <Select v-model="settings.outputDevice" :disabled="stream.isStreaming">
              <SelectOption
                v-if="settings.outputDevices.length === 0"
                value=""
                :label="t('connection.noOutputDevices')"
              />
              <SelectOption v-for="d in settings.outputDevices" :key="d" :value="d" :label="d" />
            </Select>
          </Field>

          <Field :label="t('connection.sourceAddress')">
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

          <Field :label="t('connection.psk')" :help="t('connection.pskHelp')">
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
import { onMounted } from "vue";
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

const { t } = useI18n();
const settings = useSettingsStore();
const stream = useStreamStore();

const roleOptions = [
  { value: "source", label: t("connection.roleSource") },
  { value: "sink", label: t("connection.roleSink") },
];

onMounted(async () => {
  await settings.loadOutputDevices();
});

async function onLoopbackChange() {
  await settings.loadDevices();
}
</script>
