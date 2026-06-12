<template>
  <div class="flex flex-col gap-4">
    <!-- Profiles -->
    <Card :title="t('settings.profiles')">
      <div class="flex flex-col gap-3">
        <Field :label="t('settings.selectProfile')" :tooltip="t('settings.help.profile')">
          <Select
            :model-value="settings.currentProfile"
            @update:model-value="(v: string | number) => settings.switchProfile(String(v))"
          >
            <SelectOption
              v-for="(_, name) in settings.profiles"
              :key="name"
              :value="name"
              :label="name"
            />
          </Select>
        </Field>

        <div class="flex gap-2">
          <Button size="sm" variant="secondary" @click="saveProfile">
            {{ t("action.save") }}
          </Button>
          <Button size="sm" variant="secondary" @click="openNewDialog = true">
            {{ t("action.new") }}
          </Button>
          <Button
            size="sm"
            variant="danger"
            :disabled="settings.currentProfile === 'Default'"
            @click="deleteProfile"
          >
            {{ t("action.delete") }}
          </Button>
        </div>
      </div>
    </Card>

    <!-- New Profile Dialog -->
    <Dialog :open="openNewDialog" :title="t('action.create')" @update:open="openNewDialog = $event">
      <div class="flex flex-col gap-4">
        <Input v-model="newProfileName" placeholder="Profile name" @keyup.enter="confirmCreate" />
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

    <!-- Automation -->
    <Card :title="t('settings.automation')">
      <div class="flex flex-col gap-4">
        <div class="flex items-center justify-between">
          <SettingLabel :label="t('settings.autoStart')" :tooltip="t('settings.help.autoStart')" />
          <Switch v-model="settings.autostart" @update:model-value="settings.toggleAutostart()" />
        </div>
        <div class="flex items-center justify-between">
          <SettingLabel
            :label="t('settings.autoStream')"
            :tooltip="t('settings.help.autoStream')"
          />
          <Switch v-model="settings.autoStream" />
        </div>
        <div
          v-if="settings.role === 'source' && !settings.isServer"
          class="flex items-center justify-between"
        >
          <SettingLabel
            :label="t('settings.autoReconnect')"
            :tooltip="t('settings.help.autoReconnect')"
          />
          <Switch v-model="settings.autoReconnect" />
        </div>
      </div>
    </Card>

    <!-- Preferences -->
    <Card :title="t('settings.preferences')">
      <div class="flex items-center justify-between">
        <SettingLabel :label="t('settings.language')" :tooltip="t('settings.help.language')" />
        <Select
          :model-value="ui.locale"
          class="w-[120px]"
          @update:model-value="(v) => ui.setLocale(v as 'en' | 'es')"
        >
          <SelectOption value="en" label="English" />
          <SelectOption value="es" label="Español" />
        </Select>
      </div>
    </Card>

    <!-- About -->
    <Card :title="t('settings.about')">
      <div class="flex flex-col gap-3">
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
          href="https://www.paypal.com/donate/?hosted_button_id=YOUR_ID"
          target="_blank"
        >
          {{ t("settings.donate") }}
        </Button>
      </div>
    </Card>
  </div>
</template>
<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settings";
import { useStreamStore } from "../../stores/stream";
import { useUiStore } from "../../stores/ui";
import Card from "../ui/Card.vue";
import Field from "../ui/Field.vue";
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
const newProfileName = ref("");

async function saveProfile() {
  await settings.saveSettings();
  stream.addToast(t("toast.settingsSaved", { name: settings.currentProfile }), "success");
}

async function confirmCreate() {
  if (!newProfileName.value) return;
  const ok = await settings.createProfile(newProfileName.value);
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
  const ok = await settings.deleteProfile(name);
  if (ok) {
    stream.addToast(t("toast.profileDeleted", { name }), "success");
  } else {
    stream.addToast(t("toast.cannotDeleteDefault"), "error");
  }
}
</script>
