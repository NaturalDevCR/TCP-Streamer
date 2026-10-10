<template>
  <fieldset :disabled="settings.configurationLocked" class="flex flex-col gap-4 min-w-0">
    <Card>
      <Field :label="t('connection.role')" :tooltip="t('connection.help.role')">
        <SegmentedControl
          v-model="settings.role"
          :options="roleOptions"
          :disabled="settings.configurationLocked"
        />
      </Field>
    </Card>

    <p class="text-xs text-[var(--color-text-muted)]">{{ t("operation.onboarding") }}</p>
    <!-- SOURCE -->
    <template v-if="settings.role === 'source'">
      <Card :title="t('nav.connection')">
        <div class="flex flex-col gap-4">
          <Field :label="t('audio.inputDevice')" :tooltip="t('connection.help.inputDevice')">
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

          <Button
            size="sm"
            variant="secondary"
            :disabled="settings.devicesLoading"
            @click="settings.loadDevices()"
            >{{ t("operation.refresh") }}</Button
          >
          <p v-if="!settings.devices.length" class="text-xs text-[var(--color-text-muted)]">
            {{ t("operation.microphone") }}
          </p>
          <p v-if="settings.devicesError" class="text-xs text-[var(--color-danger)]">
            {{ settings.devicesError }}
          </p>

          <template v-if="settings.showLoopback">
            <div class="flex items-center justify-between">
              <SettingLabel :label="t('audio.loopback')" :tooltip="t('connection.help.loopback')" />
              <Switch
                v-model="settings.loopbackMode"
                :aria-label="t('audio.loopback')"
                @update:model-value="onLoopbackChange"
              />
            </div>
          </template>

          <Field :label="t('connection.transport')" :tooltip="t('connection.help.transport')">
            <Select v-model="settings.transport" :disabled="settings.configurationLocked">
              <SelectOption value="tcp" :label="t('connection.transportTcp')" />
              <SelectOption value="udp" :label="t('connection.transportUdp')" />
            </Select>
          </Field>

          <Field
            v-if="settings.transport === 'tcp'"
            :label="t('connection.mode')"
            :tooltip="t('connection.help.mode')"
          >
            <Select v-model="settings.mode" :disabled="settings.configurationLocked">
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
              :disabled="settings.configurationLocked"
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
                :disabled="settings.configurationLocked"
              />
            </Field>
            <Field
              :label="settings.isServer ? t('connection.listeningPort') : t('connection.port')"
              :tooltip="t('connection.help.port')"
              :class="settings.isServer ? 'flex-1' : 'w-[120px]'"
            >
              <Input
                v-model.number="settings.port"
                type="number"
                :disabled="settings.configurationLocked"
              />
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
              :disabled="settings.configurationLocked"
            />
          </Field>

          <Card
            v-if="settings.isServer && settings.configurationLocked"
            :title="t('connection.info')"
          >
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
            <Input
              v-model.number="settings.port"
              type="number"
              :disabled="settings.configurationLocked"
            />
          </Field>
        </div>
      </Card>
    </template>

    <div
      v-if="settings.transport === 'udp' || settings.role === 'sink'"
      class="flex items-center justify-between gap-3"
    >
      <div>
        <SettingLabel :label="t('operation.rememberSecret')" />
        <p class="text-xs text-[var(--color-text-muted)]">{{ t("operation.secret") }}</p>
      </div>
      <Switch v-model="settings.rememberPsk" :aria-label="t('operation.rememberSecret')" />
    </div>
    <!-- SINK -->
    <template v-if="settings.role === 'sink'">
      <Card :title="t('connection.roleSink')">
        <div class="flex flex-col gap-4">
          <Field :label="t('connection.outputDevice')" :tooltip="t('connection.help.outputDevice')">
            <Select
              v-model="settings.outputDevice"
              :placeholder="t('connection.noOutputDevices')"
              :disabled="settings.configurationLocked"
            >
              <SelectOption v-for="d in settings.outputDevices" :key="d" :value="d" :label="d" />
            </Select>
          </Field>

          <Button
            size="sm"
            variant="secondary"
            :disabled="settings.outputDevicesLoading"
            @click="settings.loadOutputDevices()"
            >{{ t("operation.refresh") }}</Button
          >
          <p
            v-if="settings.outputDevicesError"
            role="alert"
            class="text-xs text-[var(--color-danger)]"
          >
            {{ settings.outputDevicesError }}
          </p>
          <Field
            :label="t('connection.sourceAddress')"
            :tooltip="t('connection.help.sourceAddress')"
          >
            <Input
              v-model="settings.sourceAddr"
              :placeholder="t('connection.sourceAddressPlaceholder')"
              :disabled="settings.configurationLocked"
            />
          </Field>

          <div class="flex gap-2 items-end">
            <Button
              size="sm"
              variant="secondary"
              :disabled="settings.configurationLocked || settings.scanLoading"
              @click="settings.scanSources()"
            >
              {{ settings.scanLoading ? t("operation.searching") : t("action.scan") }}
            </Button>
            <Button
              v-if="settings.scanLoading"
              size="sm"
              variant="ghost"
              @click="settings.cancelScan()"
              >{{ t("action.cancel") }}</Button
            >
            <span
              v-if="settings.discoveredSources.length"
              class="text-xs text-[var(--color-text-muted)] self-center"
            >
              {{ t("operation.found", { count: settings.discoveredSources.length }) }}
            </span>
          </div>

          <p v-if="settings.scanError" role="alert" class="text-xs text-[var(--color-danger)]">
            {{ settings.scanError }}
          </p>
          <p
            v-else-if="settings.scanCompleted && !settings.discoveredSources.length"
            role="status"
            class="text-xs"
          >
            {{ t("operation.noSources") }}
          </p>
          <Field
            v-if="settings.discoveredSources.length"
            :label="t('connection.discoveredSources')"
            :tooltip="t('connection.help.discoveredSources')"
          >
            <Select
              :model-value="settings.sourceAddr"
              :disabled="settings.configurationLocked"
              @update:model-value="(v: string | number) => (settings.sourceAddr = String(v))"
            >
              <SelectOption
                v-for="s in settings.discoveredSources"
                :key="s.addr"
                :value="s.addr"
                :label="`${s.name} (${s.addr})${s.encrypted ? ` [${t('operation.encrypted')}]` : ''}`"
              />
            </Select>
          </Field>

          <Field :label="t('connection.psk')" :tooltip="t('connection.help.psk')">
            <Input
              v-model="settings.psk"
              type="password"
              :placeholder="t('connection.pskPlaceholder')"
              :disabled="settings.configurationLocked"
            />
          </Field>
        </div>
      </Card>
    </template>
    <Card :title="t('preflight.title')">
      <p class="text-xs text-[var(--color-text-muted)]">{{ t("preflight.scopeNote") }}</p>
      <Button
        size="sm"
        class="mt-2"
        :disabled="settings.preflightLoading"
        @click="settings.preflight()"
        >{{ t(settings.preflightLoading ? "preflight.checking" : "preflight.check") }}</Button
      >
      <p
        v-if="settings.preflightError"
        role="alert"
        class="text-sm text-[var(--color-danger)] mt-2"
      >
        {{ settings.preflightError }}
      </p>
      <div v-if="settings.preflightResult" class="mt-3 space-y-2" role="status">
        <p v-if="settings.preflightStale" class="text-xs text-[var(--color-warning)]">
          {{ t("preflight.stale") }}
        </p>
        <p v-else class="text-sm">
          {{ t(settings.preflightResult.ready ? "preflight.ready" : "preflight.notReady") }}
        </p>
        <div v-for="stage in settings.preflightResult.stages" :key="stage.name" class="text-xs">
          <span
            :class="
              stage.status === 'error'
                ? 'text-[var(--color-danger)]'
                : stage.status === 'warning'
                  ? 'text-[var(--color-warning)]'
                  : 'text-[var(--color-success)]'
            "
            >{{ t(`preflight.stages.${stage.name}`) }} ·
            {{ t(`preflight.status.${stage.status}`) }}</span
          >
          <p class="text-[var(--color-text-muted)]">{{ stage.detail }}</p>
        </div>
      </div>
    </Card>
  </fieldset>
</template>
<script setup lang="ts">
import { computed } from "vue";
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

async function onLoopbackChange() {
  await settings.loadDevices();
}
</script>
