<template>
  <fieldset :disabled="settings.configurationLocked" class="configuration-grid settings-grid">
    <p
      v-if="settings.persistenceError"
      role="alert"
      class="span-full text-sm text-[var(--color-danger)]"
    >
      {{ settings.persistenceError }}
    </p>
    <p v-if="settings.isDirty" role="status" class="span-full notice text-xs">
      {{ t("operation.dirty") }}
    </p>
    <!-- Profiles -->
    <Card
      class="span-full"
      :title="t('settings.profiles')"
      :description="t('workspace.profilesDescription')"
    >
      <div class="profile-controls">
        <Field :label="t('settings.selectProfile')" :tooltip="t('settings.help.profile')">
          <Select
            :model-value="settings.currentProfile"
            :disabled="!settings.nativeAvailable"
            @update:model-value="(v: string | number) => switchProfile(String(v))"
          >
            <SelectOption
              v-for="(_, name) in settings.profiles"
              :key="name"
              :value="name"
              :label="name"
            />
          </Select>
        </Field>

        <div class="flex flex-wrap gap-2">
          <Button
            size="sm"
            variant="secondary"
            :disabled="!settings.nativeAvailable"
            @click="saveProfile"
          >
            {{ t("action.save") }}
          </Button>
          <Button
            size="sm"
            variant="secondary"
            :disabled="!settings.nativeAvailable"
            @click="openNewDialog = true"
          >
            {{ t("action.new") }}
          </Button>
          <Button
            size="sm"
            variant="danger"
            :disabled="!settings.nativeAvailable || settings.currentProfile === 'Default'"
            @click="deleteOpen = true"
          >
            {{ t("action.delete") }}
          </Button>
        </div>
      </div>
      <div class="profile-transfer">
        <div>
          <h3>{{ t("workspace.profileTransfer") }}</h3>
          <p>{{ t("workspace.profileTransferDescription") }}</p>
        </div>
        <div class="flex flex-wrap gap-2">
          <Button size="sm" variant="secondary" @click="exportProfile">{{
            t("operation.export")
          }}</Button
          ><Button
            size="sm"
            variant="secondary"
            :disabled="!settings.nativeAvailable"
            @click="importOpen = true"
            >{{ t("operation.import") }}</Button
          >
        </div>
      </div>
    </Card>

    <Dialog
      :open="Boolean(pendingProfile)"
      :title="t('operation.dirty')"
      :description="t('operation.switchConfirm')"
      @update:open="pendingProfile = ''"
    >
      <div class="flex justify-end gap-2">
        <Button variant="ghost" @click="pendingProfile = ''">{{ t("action.cancel") }}</Button
        ><Button @click="confirmSwitch">{{ t("operation.save") }}</Button>
      </div>
    </Dialog>
    <Dialog
      :open="deleteOpen"
      :title="t('action.delete')"
      :description="t('operation.deleteConfirm', { name: settings.currentProfile })"
      @update:open="deleteOpen = $event"
    >
      <div class="flex justify-end gap-2">
        <Button variant="ghost" @click="deleteOpen = false">{{ t("action.cancel") }}</Button
        ><Button variant="danger" @click="deleteProfile">{{ t("action.delete") }}</Button>
      </div>
    </Dialog>
    <!-- New Profile Dialog -->
    <Dialog :open="openNewDialog" :title="t('action.create')" @update:open="openNewDialog = $event">
      <div class="flex flex-col gap-4">
        <Input
          v-model="newProfileName"
          :placeholder="t('operation.profileName')"
          :aria-label="t('operation.profileName')"
          @keyup.enter="confirmCreate"
        />
        <div class="flex justify-end gap-2">
          <Button
            variant="ghost"
            size="sm"
            @click="
              openNewDialog = false;
              newProfileName = '';
            "
          >
            {{ t("action.cancel") }}
          </Button>
          <Button size="sm" @click="confirmCreate">
            {{ t("action.create") }}
          </Button>
        </div>
      </div>
    </Dialog>

    <Dialog :open="importOpen" :title="t('operation.import')" @update:open="importOpen = $event">
      <Field :label="t('operation.profileName')"><Input v-model="importName" /></Field>
      <Field :label="t('operation.importHelp')"
        ><Textarea v-model="importContent" :rows="8"
      /></Field>
      <Button @click="importProfile">{{ t("operation.import") }}</Button>
    </Dialog>
    <!-- Automation -->
    <Card :title="t('settings.automation')" :description="t('workspace.automationDescription')">
      <div class="flex flex-col gap-4">
        <div class="flex items-center justify-between">
          <SettingLabel :label="t('settings.autoStart')" :tooltip="t('settings.help.autoStart')" />
          <Switch
            v-model="settings.autostart"
            :disabled="!settings.nativeAvailable"
            :aria-label="t('settings.autoStart')"
            @update:model-value="settings.toggleAutostart()"
          />
        </div>
        <div class="flex items-center justify-between">
          <SettingLabel
            :label="t('settings.autoStream')"
            :tooltip="t('settings.help.autoStream')"
          />
          <Switch v-model="settings.autoStream" :aria-label="t('settings.autoStream')" />
        </div>
        <div
          v-if="settings.role === 'source' && settings.transport === 'tcp' && !settings.isServer"
          class="flex items-center justify-between"
        >
          <SettingLabel
            :label="t('settings.autoReconnect')"
            :tooltip="t('settings.help.autoReconnect')"
          />
          <Switch v-model="settings.autoReconnect" :aria-label="t('settings.autoReconnect')" />
        </div>
      </div>
    </Card>

    <!-- Preferences -->
    <Card :title="t('settings.preferences')">
      <div class="flex items-center justify-between">
        <SettingLabel :label="t('settings.language')" :tooltip="t('settings.help.language')" />
        <Select
          :model-value="ui.locale"
          :aria-label="t('settings.language')"
          class="w-[120px]"
          @update:model-value="(v) => ui.setLocale(v as 'en' | 'es')"
        >
          <SelectOption value="en" label="English" />
          <SelectOption value="es" label="Español" />
        </Select>
      </div>
    </Card>

    <!-- About -->
    <Card class="span-full about-card" :title="t('settings.about')">
      <div class="flex flex-col gap-4">
        <p class="text-sm text-[var(--color-text)]">
          {{ t("app.name") }}
          <span class="text-[var(--color-text-muted)]">v{{ version }}</span>
        </p>
        <p class="text-xs text-[var(--color-text-muted)]">
          {{ t("settings.madeBy") }}
        </p>
        <Button
          size="sm"
          variant="secondary"
          as="a"
          href="https://github.com/NaturalDevCR/TCP-Streamer/issues"
          target="_blank"
          @click="openSupport"
        >
          {{ t("operation.support") }}
        </Button>
      </div>
    </Card>
  </fieldset>
</template>
<script setup lang="ts">
import { openUrl } from "@tauri-apps/plugin-opener";
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settings";
import { useStreamStore } from "../../stores/stream";
import { useUiStore } from "../../stores/ui";
import Card from "../ui/Card.vue";
import Field from "../ui/Field.vue";
import Textarea from "../ui/Textarea.vue";
import Input from "../ui/Input.vue";
import Button from "../ui/Button.vue";
import Select from "../ui/Select.vue";
import SelectOption from "../ui/SelectOption.vue";
import Switch from "../ui/Switch.vue";
import Dialog from "../ui/Dialog.vue";
import SettingLabel from "../ui/SettingLabel.vue";

const { t } = useI18n();
const settings = useSettingsStore();
const stream = useStreamStore();
const ui = useUiStore();
const version = import.meta.env.PACKAGE_VERSION || "0.0.0";

const openNewDialog = ref(false);
const deleteOpen = ref(false);
const pendingProfile = ref("");
const newProfileName = ref("");
const importOpen = ref(false),
  importName = ref(""),
  importContent = ref("");
async function exportProfile() {
  try {
    await navigator.clipboard.writeText(settings.exportProfile());
    stream.addToast(t("toast.copied"));
  } catch {
    stream.addToast(t("toast.copyFailed"), "error");
  }
}
async function importProfile() {
  try {
    if (await settings.importProfile(importName.value, importContent.value)) {
      importOpen.value = false;
      stream.addToast(t("toast.settingsSaved", { name: importName.value }));
    } else stream.addToast(t("toast.profileExists"), "error");
  } catch (e) {
    stream.addToast(String(e), "error");
  }
}
async function switchProfile(name: string) {
  if (settings.isDirty) {
    pendingProfile.value = name;
    return;
  }
  try {
    await settings.switchProfile(name);
  } catch (e) {
    stream.addToast(String(e), "error");
  }
}

async function confirmSwitch() {
  try {
    await settings.switchProfile(pendingProfile.value);
    pendingProfile.value = "";
  } catch (e) {
    stream.addToast(String(e), "error");
  }
}
async function openSupport(event: MouseEvent) {
  if (!settings.nativeAvailable) return;
  event.preventDefault();
  try {
    await openUrl("https://github.com/NaturalDevCR/TCP-Streamer/issues");
  } catch (e) {
    stream.addToast(String(e), "error");
  }
}
async function saveProfile() {
  if (!(await settings.saveSettings())) {
    stream.addToast(settings.persistenceError, "error");
    return;
  }
  stream.addToast(t("toast.settingsSaved", { name: settings.currentProfile }), "success");
}

async function confirmCreate() {
  if (!newProfileName.value) return;
  let ok: boolean;
  try {
    ok = await settings.createProfile(newProfileName.value);
  } catch (e) {
    stream.addToast(String(e), "error");
    return;
  }
  if (ok) {
    stream.addToast(t("toast.profileCreated", { name: newProfileName.value }), "success");
    newProfileName.value = "";
    openNewDialog.value = false;
  } else {
    stream.addToast(t("toast.profileExists"), "error");
  }
}

async function deleteProfile() {
  const name = settings.currentProfile;
  deleteOpen.value = false;
  let ok: boolean;
  try {
    ok = await settings.deleteProfile(name);
  } catch (e) {
    stream.addToast(String(e), "error");
    return;
  }
  if (ok) {
    stream.addToast(t("toast.profileDeleted", { name }), "success");
  } else {
    stream.addToast(t("toast.cannotDeleteDefault"), "error");
  }
}
</script>
