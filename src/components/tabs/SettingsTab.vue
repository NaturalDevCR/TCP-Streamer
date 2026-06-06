<template>
  <div class="animate-fade-in space-y-4">
    <!-- Profiles -->
    <section class="glass-card">
      <h3 class="text-sm font-semibold text-slate-300 border-b border-white/10 pb-2 mb-4">
        Configuration Profiles
      </h3>

      <SelectField
        id="profile-select"
        :model-value="settings.currentProfile"
        label="Select Profile"
        @update:model-value="settings.switchProfile($event)"
      >
        <option v-for="(_, name) in settings.profiles" :key="name" :value="name">{{ name }}</option>
      </SelectField>

      <div class="flex gap-2 mt-3">
        <button class="btn-sm bg-white/10 hover:bg-white/20" @click="saveProfile">💾 Save</button>
        <button class="btn-sm bg-white/10 hover:bg-white/20" @click="showNewProfile = true">
          ➕ New
        </button>
        <button
          class="btn-sm bg-red-500/20 text-red-300 hover:bg-red-500/30"
          :disabled="settings.currentProfile === 'Default'"
          @click="deleteProfile"
        >
          🗑️ Delete
        </button>
      </div>

      <!-- New Profile Input -->
      <div v-if="showNewProfile" class="flex gap-2 mt-3">
        <input
          v-model="newName"
          placeholder="Enter Profile Name"
          class="flex-1 px-3 py-1.5 bg-black/20 border border-white/10 rounded-lg text-slate-50 text-sm outline-none focus:border-accent"
          @keyup.enter="confirmCreate"
        />
        <button
          class="btn-sm bg-emerald-500/20 text-emerald-300 hover:bg-emerald-500/30"
          @click="confirmCreate"
        >
          ✓ Create
        </button>
        <button
          class="btn-sm bg-white/10 hover:bg-white/20"
          @click="
            showNewProfile = false;
            newName = '';
          "
        >
          ✕ Cancel
        </button>
      </div>
    </section>

    <!-- Automation -->
    <section class="glass-card">
      <h3 class="text-sm font-semibold text-slate-300 border-b border-white/10 pb-2 mb-4">
        Automation
      </h3>
      <div class="space-y-3">
        <CheckboxField
          v-model="settings.autostart"
          label="Auto-start on launch"
          @update:model-value="settings.toggleAutostart"
        />
        <CheckboxField v-model="settings.autoStream" label="Auto-stream on load" />
        <div v-if="!settings.isServer">
          <CheckboxField v-model="settings.autoReconnect" label="Auto-Reconnect" />
        </div>
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { useSettingsStore } from "../../stores/settings.ts";
import { useStreamStore } from "../../stores/stream.ts";
import SelectField from "../ui/SelectField.vue";
import CheckboxField from "../ui/CheckboxField.vue";

const settings = useSettingsStore();
const stream = useStreamStore();

const showNewProfile = ref(false);
const newName = ref("");

async function saveProfile() {
  await settings.saveSettings();
  stream.addToast(`Settings saved to '${settings.currentProfile}'`, "success");
}

async function confirmCreate() {
  if (!newName.value) return;
  const ok = await settings.createProfile(newName.value);
  if (ok) {
    stream.addToast(`Profile '${newName.value}' created`, "success");
    newName.value = "";
    showNewProfile.value = false;
  } else {
    stream.addToast("Profile already exists", "error");
  }
}

async function deleteProfile() {
  const name = settings.currentProfile;
  const ok = await settings.deleteProfile(name);
  if (ok) {
    stream.addToast(`Profile '${name}' deleted`, "success");
  } else {
    stream.addToast("Cannot delete Default profile", "error");
  }
}
</script>

<style scoped>
.btn-sm {
  padding: 6px 12px;
  border: none;
  border-radius: 6px;
  color: var(--color-slate-50);
  font-size: 0.8rem;
  cursor: pointer;
  transition: background 0.2s;
}
.btn-sm:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
</style>
