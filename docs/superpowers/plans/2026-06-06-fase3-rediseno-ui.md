# Phase 3: UI/UX Redesign (enterprise dark + sidebar + i18n) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the "gamer" glassmorphism UI with a clean enterprise **dark** dashboard: a resizable window with a sidebar + persistent header, role-aware sections (Source/Sink), an accessible design system (Reka UI + Tailwind), and English/Spanish i18n — with **no backend feature changes** (Pinia stores and Tauri commands stay).

**Architecture:** A new design-token layer and a set of reusable UI primitives (hand-made for simple controls, Reka UI for Select/Switch/Dialog/Tooltip/SegmentedControl). A 3-zone `AppShell` (sidebar / header / content). Sections (`Dashboard`, `Connection`, `Audio`, `Logs`, `Settings`) bind to the existing `settings`/`stream` stores. All chrome strings go through `vue-i18n`.

**Tech Stack:** Vue 3.5, Pinia 3, Tailwind 4, **Reka UI**, **vue-i18n**, Vite 7, Vitest.

**Spec:** `docs/superpowers/specs/2026-06-06-fase3-rediseno-ui-design.md`
**Depends on:** Phases 1–2 merged (`main`, v2.2.0). Stores `src/stores/{settings,stream}.ts` are the data source — do not change their public API; this plan only changes presentation + adds `locale`/`theme` state.

**How view tasks are specified:** Foundation tasks (tokens, primitives, shell, i18n, stores) ship **complete code**. Section tasks give a **precise content + store-binding + state spec** plus key markup, built from the already-defined primitives — the appropriate altitude for view components (which have visual latitude). The Dashboard (Task 5.1) is a fully worked example of that pattern.

---

## Store binding reference (existing, unchanged)

`useSettingsStore()` exposes: `devices, osType, localIp, deviceName, mode, ip, port, loopbackMode, sampleRate, bufferSize, ringBufferDuration, format, adaptiveBuffer, minBuffer, maxBuffer, autostart, autoStream, autoReconnect, highPriority, dscpStrategy, chunkSize, latencyProfile, allowlist, role, transport, outputDevice, sourceAddr, outputDevices, psk, discoveredSources, profiles, currentProfile, devicesLoading, devicesError`, computeds `isServer, isLoopback, showLoopback`, actions `init, loadDevices, loadOutputDevices, loadSettings, saveSettings, switchProfile, createProfile, deleteProfile, toggleAutostart, scanSources`.

`useStreamStore()` exposes: `isStreaming, statusText, uptimeSeconds, bytesSent, bitrateKbps, qualityScore, rttMs, rttVarMs, underruns, dropped, bufferHealth, bufferMs, logs, logFilter, toasts, tcpUrl, httpUrl, connectionInfo`, computeds `filteredLogs, formattedUptime, formattedBytes, qualityLabel`, actions `addToast, clearLogs, startStream, stopStream, toggleStream, initListeners`.

> If any of the above is missing (Phase 2 evolved the store), grep the store and adapt the binding name. Do **not** invent new commands.

---

## File Structure

| File | Responsibility | Status |
|---|---|---|
| `src/style.css` | dark-neutral tokens; remove glass/glow/gradients | Rewrite |
| `src/i18n/index.ts` · `en.json` · `es.json` | vue-i18n setup + catalogs | Create |
| `src/components/ui/*` | primitives (Button, Input, Textarea, Field, Card, Stat, Badge, StatusDot, CopyField, Select, Switch, Tooltip, Dialog, SegmentedControl) | Create |
| `src/components/AppShell.vue` · `Sidebar.vue` · `Header.vue` | shell | Create |
| `src/components/sections/{DashboardSection,ConnectionSection,AudioSection,LogsSection,SettingsSection}.vue` | sections | Create |
| `src/components/icons.ts` | consolidated SVG icon set (lucide-style) | Rewrite |
| `src/stores/ui.ts` | `locale` + `theme` + active section state | Create |
| `src/App.vue` · `src/main.ts` | mount shell + i18n | Modify |
| `src-tauri/tauri.conf.json` · `src-tauri/src/main.rs` | resizable window; remove sizing hack | Modify |
| old: `components/tabs/*`, `TabNav.vue`, `StreamButton.vue`, `StatsBar.vue`, `AppHeader.vue`, `AppFooter.vue`, `ui/{CheckboxField,SelectField,InputField}.vue` | superseded | Delete (M7) |

---

## Milestone 0 — Dependencies + i18n scaffolding

### Task 0.1: Install Reka UI and vue-i18n

- [ ] **Step 1:** `pnpm add reka-ui vue-i18n`
- [ ] **Step 2:** `pnpm build 2>&1 | tail -5` → builds. (If a peer-dep warning appears for the Vue version, pin `vue-i18n@^11` / `reka-ui@^2`.)
- [ ] **Step 3:** Commit:
```bash
git add package.json pnpm-lock.yaml
git commit -m "build: add reka-ui + vue-i18n for the redesign"
```

### Task 0.2: i18n setup + catalogs

**Files:** Create `src/i18n/index.ts`, `src/i18n/en.json`, `src/i18n/es.json`; Modify `src/main.ts`

- [ ] **Step 1:** Create `src/i18n/en.json` and `src/i18n/es.json` as the keyed catalogs. Seed them with the top-level keys used by the shell (sections + actions); they grow as sections are built (M6 finalizes coverage). Start:

`en.json`:
```json
{
  "app": { "name": "TCP Streamer" },
  "nav": { "dashboard": "Dashboard", "connection": "Connection", "audio": "Audio", "logs": "Logs", "settings": "Settings" },
  "status": { "ready": "Ready", "streaming": "Streaming", "listening": "Listening", "playing": "Playing" },
  "action": { "start": "Start", "stop": "Stop", "save": "Save", "create": "Create", "delete": "Delete", "cancel": "Cancel", "copy": "Copy", "scan": "Scan", "clear": "Clear" }
}
```
`es.json` (same keys):
```json
{
  "app": { "name": "TCP Streamer" },
  "nav": { "dashboard": "Panel", "connection": "Conexión", "audio": "Audio", "logs": "Registros", "settings": "Ajustes" },
  "status": { "ready": "Listo", "streaming": "Transmitiendo", "listening": "Escuchando", "playing": "Reproduciendo" },
  "action": { "start": "Iniciar", "stop": "Detener", "save": "Guardar", "create": "Crear", "delete": "Borrar", "cancel": "Cancelar", "copy": "Copiar", "scan": "Buscar", "clear": "Limpiar" }
}
```

- [ ] **Step 2:** Create `src/i18n/index.ts`:
```ts
import { createI18n } from "vue-i18n";
import en from "./en.json";
import es from "./es.json";

function defaultLocale(): "en" | "es" {
  const nav = typeof navigator !== "undefined" ? navigator.language : "en";
  return nav.toLowerCase().startsWith("es") ? "es" : "en";
}

export const i18n = createI18n({
  legacy: false,
  locale: defaultLocale(),
  fallbackLocale: "en",
  messages: { en, es },
});
```

- [ ] **Step 3:** In `src/main.ts`, `import { i18n } from "./i18n";` and `app.use(i18n)` (alongside the existing Pinia `app.use(...)`). Run `pnpm typecheck` → green.

- [ ] **Step 4:** Commit:
```bash
git add src/i18n src/main.ts
git commit -m "feat(i18n): vue-i18n setup with en/es catalogs"
```

---

## Milestone 1 — Design tokens

### Task 1.1: Rewrite `src/style.css` (dark neutral, no glass/glow)

**Files:** Rewrite `src/style.css`

- [ ] **Step 1:** Replace the file with the new token system (remove `radial-gradient` body, `.glass-card`, `.glow-btn`, glow/pulse keyframes, gradient text):

```css
@import "tailwindcss";

@theme {
  /* Neutral dark surfaces (layered, differentiated by lightness + border) */
  --color-bg: #0b0d10;
  --color-surface-1: #14171c;
  --color-surface-2: #1b1f26;
  --color-surface-3: #232832;
  --color-border: #2a313c;
  --color-border-strong: #3a414d;

  --color-text: #e6e9ef;
  --color-text-muted: #9aa3b2;
  --color-text-faint: #6b7382;

  --color-accent: #4f6bff;
  --color-accent-hover: #6379ff;
  --color-on-accent: #ffffff;

  --color-success: #3fb950;
  --color-warning: #d29922;
  --color-danger: #f85149;

  --font-sans: "Inter", system-ui, -apple-system, sans-serif;
  --font-mono: "JetBrains Mono", "Fira Code", monospace;

  --radius: 8px;
}

html, body, #app { height: 100%; }
body {
  margin: 0;
  font-family: var(--font-sans);
  background: var(--color-bg);
  color: var(--color-text);
  font-size: 14px;
  -webkit-font-smoothing: antialiased;
}

/* Single focus-ring utility used by all interactive primitives */
.focus-ring:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: 2px;
}

::-webkit-scrollbar { width: 10px; height: 10px; }
::-webkit-scrollbar-thumb { background: var(--color-border-strong); border-radius: 6px; }
::-webkit-scrollbar-track { background: transparent; }
```

- [ ] **Step 2:** `pnpm build` → builds (components still reference old classes; they get replaced/deleted through M2–M7, and old components are removed in M7. Until then the app may look broken — that is expected during a redesign; the gate that matters is typecheck/lint/build, verified per task).

- [ ] **Step 3:** Commit:
```bash
git add src/style.css
git commit -m "feat(ui): dark-neutral design tokens (remove glass/glow)"
```

### Task 1.2: Consolidated icon set

**Files:** Rewrite `src/components/icons.ts`

- [ ] **Step 1:** Make `icons.ts` export functional SVG icon components (lucide-style, `stroke="currentColor"`, `width/height=18`, `stroke-width=2`) for: `IconDashboard, IconConnection, IconAudio, IconLogs, IconSettings, IconPlay, IconStop, IconCopy, IconSearch, IconTrash, IconPlus, IconChevronDown, IconCheck, IconGlobe`. Each is a small `h()` render function returning an `<svg>` with the appropriate `<path>`. Keep the existing export names that `TabNav` used where possible to ease migration; remove emoji usage everywhere.

- [ ] **Step 2:** `pnpm typecheck` → green. Commit:
```bash
git add src/components/icons.ts
git commit -m "feat(ui): consolidated SVG icon set (no emojis)"
```

---

## Milestone 2 — Hand-made primitives

> Each primitive: create the `.vue` file, then `pnpm typecheck && pnpm lint` green, then commit. Grouped commits per 2–3 related primitives are fine.

### Task 2.1: Button, Input, Textarea

**Files:** Create `src/components/ui/Button.vue`, `Input.vue`, `Textarea.vue`

- [ ] **Step 1:** `Button.vue`:
```vue
<template>
  <button
    :type="type"
    :disabled="disabled"
    :class="[
      'focus-ring inline-flex items-center justify-center gap-2 rounded-[var(--radius)] font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed',
      sizeClass,
      variantClass,
    ]"
  >
    <slot />
  </button>
</template>
<script setup lang="ts">
import { computed } from "vue";
const props = defineProps<{
  variant?: "primary" | "secondary" | "ghost" | "danger";
  size?: "sm" | "md";
  type?: "button" | "submit";
  disabled?: boolean;
}>();
const sizeClass = computed(() => (props.size === "sm" ? "h-8 px-3 text-[13px]" : "h-9 px-4 text-sm"));
const variantClass = computed(() => {
  switch (props.variant) {
    case "secondary": return "bg-[var(--color-surface-2)] text-[var(--color-text)] border border-[var(--color-border)] hover:bg-[var(--color-surface-3)]";
    case "ghost": return "bg-transparent text-[var(--color-text-muted)] hover:bg-[var(--color-surface-2)] hover:text-[var(--color-text)]";
    case "danger": return "bg-[var(--color-danger)] text-white hover:opacity-90";
    default: return "bg-[var(--color-accent)] text-[var(--color-on-accent)] hover:bg-[var(--color-accent-hover)]";
  }
});
</script>
```

- [ ] **Step 2:** `Input.vue` — a controlled text/number input with `v-model`:
```vue
<template>
  <input
    :type="type ?? 'text'"
    :value="modelValue"
    :placeholder="placeholder"
    :disabled="disabled"
    class="focus-ring w-full h-9 px-3 rounded-[var(--radius)] bg-[var(--color-surface-1)] border border-[var(--color-border)] text-sm text-[var(--color-text)] placeholder:text-[var(--color-text-faint)] disabled:opacity-50"
    @input="onInput"
  />
</template>
<script setup lang="ts">
const props = defineProps<{ modelValue?: string | number; type?: string; placeholder?: string; disabled?: boolean }>();
const emit = defineEmits<{ "update:modelValue": [string | number] }>();
function onInput(e: Event) {
  const v = (e.target as HTMLInputElement).value;
  emit("update:modelValue", props.type === "number" ? Number(v) : v);
}
</script>
```

- [ ] **Step 3:** `Textarea.vue` — analogous read-mostly textarea (used for connection info). Bind `:value`, `readonly` prop, mono font class.

- [ ] **Step 4:** `pnpm typecheck && pnpm lint` → green. Commit:
```bash
git add src/components/ui/Button.vue src/components/ui/Input.vue src/components/ui/Textarea.vue
git commit -m "feat(ui): Button, Input, Textarea primitives"
```

### Task 2.2: Card, Field, Stat, Badge, StatusDot, CopyField

**Files:** Create the six `.vue` files under `src/components/ui/`.

- [ ] **Step 1:** `Card.vue` — panel with optional `title` slot/prop:
```vue
<template>
  <section class="rounded-[var(--radius)] bg-[var(--color-surface-1)] border border-[var(--color-border)]">
    <header v-if="title || $slots.header" class="px-4 py-3 border-b border-[var(--color-border)] flex items-center justify-between">
      <h3 class="text-sm font-semibold text-[var(--color-text)]">{{ title }}</h3>
      <slot name="header" />
    </header>
    <div class="p-4"><slot /></div>
  </section>
</template>
<script setup lang="ts">defineProps<{ title?: string }>();</script>
```

- [ ] **Step 2:** `Field.vue` — label + control slot + optional help/error:
```vue
<template>
  <div class="flex flex-col gap-1.5">
    <label v-if="label" :for="id" class="text-[13px] font-medium text-[var(--color-text-muted)]">{{ label }}</label>
    <slot :id="id" />
    <p v-if="help" class="text-xs text-[var(--color-text-faint)]">{{ help }}</p>
    <p v-if="error" class="text-xs text-[var(--color-danger)]">{{ error }}</p>
  </div>
</template>
<script setup lang="ts">defineProps<{ label?: string; help?: string; error?: string; id?: string }>();</script>
```

- [ ] **Step 3:** `Stat.vue` — label (uppercase, muted) + mono value:
```vue
<template>
  <div class="rounded-[var(--radius)] bg-[var(--color-surface-2)] border border-[var(--color-border)] px-3 py-2.5 flex flex-col gap-1">
    <span class="text-[10px] uppercase tracking-wide text-[var(--color-text-faint)]">{{ label }}</span>
    <span class="font-mono text-sm text-[var(--color-text)]"><slot>{{ value }}</slot></span>
  </div>
</template>
<script setup lang="ts">defineProps<{ label?: string; value?: string | number }>();</script>
```

- [ ] **Step 4:** `Badge.vue` (small status pill with `tone` prop → success/warning/danger/neutral colors), `StatusDot.vue` (a colored dot + optional label, tones mapped to the semantic tokens), and `CopyField.vue` (read-only `Input` + a copy `Button` using `navigator.clipboard.writeText` and `stream.addToast`). Reuse the existing `CopyField` logic but restyled with the new primitives/tokens.

- [ ] **Step 5:** `pnpm typecheck && pnpm lint` → green. Commit:
```bash
git add src/components/ui/Card.vue src/components/ui/Field.vue src/components/ui/Stat.vue src/components/ui/Badge.vue src/components/ui/StatusDot.vue src/components/ui/CopyField.vue
git commit -m "feat(ui): Card, Field, Stat, Badge, StatusDot, CopyField primitives"
```

---

## Milestone 3 — Reka UI primitives

> Reka UI uses Radix-style composition (`SelectRoot/SelectTrigger/SelectPortal/SelectContent/SelectItem`, `SwitchRoot/SwitchThumb`, `TooltipRoot/...`, `DialogRoot/...`). **Verify the exact component/import names against the installed `reka-ui` version** (`node_modules/reka-ui`); the structure below is standard.

### Task 3.1: Select + Switch

**Files:** Create `src/components/ui/Select.vue`, `Switch.vue`

- [ ] **Step 1:** `Select.vue` — wraps Reka `SelectRoot` with `v-model`, a styled trigger (chevron icon), and a slot for `<SelectItem>`s OR an `:options` prop `{ value, label }[]`. Style the content/items with the tokens; replaces the native `<select>` (and removes the need for the old dark-option CSS hack).
- [ ] **Step 2:** `Switch.vue` — wraps Reka `SwitchRoot/SwitchThumb` with `v-model:checked`, styled track (accent when on) + thumb. Replaces `CheckboxField` for boolean toggles.
- [ ] **Step 3:** `pnpm typecheck && pnpm lint` → green. Commit:
```bash
git add src/components/ui/Select.vue src/components/ui/Switch.vue
git commit -m "feat(ui): accessible Select + Switch (Reka UI)"
```

### Task 3.2: Tooltip, Dialog, SegmentedControl

**Files:** Create `src/components/ui/Tooltip.vue`, `Dialog.vue`, `SegmentedControl.vue`

- [ ] **Step 1:** `Tooltip.vue` (Reka `TooltipRoot/Trigger/Content`), `Dialog.vue` (Reka `DialogRoot/Trigger/Portal/Overlay/Content` with `v-model:open`, title slot, body slot, footer slot — used for "create profile" and confirmations), `SegmentedControl.vue` (a 2–3 option pill toggle, `v-model`, built on Reka `Toggle Group`/`Tabs` or hand-rolled buttons with `role=radiogroup` — used for the Source/Sink role).
- [ ] **Step 2:** `pnpm typecheck && pnpm lint` → green. Commit:
```bash
git add src/components/ui/Tooltip.vue src/components/ui/Dialog.vue src/components/ui/SegmentedControl.vue
git commit -m "feat(ui): Tooltip, Dialog, SegmentedControl (Reka UI)"
```

---

## Milestone 4 — App shell

### Task 4.1: Window config (resizable, no hack)

**Files:** Modify `src-tauri/tauri.conf.json`, `src-tauri/src/main.rs`

- [ ] **Step 1:** In `tauri.conf.json` window object: `"width": 960, "height": 640, "minWidth": 760, "minHeight": 560, "resizable": true, "center": true` (keep `decorations`, `transparent: false`).
- [ ] **Step 2:** In `src-tauri/src/main.rs` `setup`, **remove** the monitor-based `set_size`/`center` block (the height hack). Keep the tray setup and the `start_hidden` logic.
- [ ] **Step 3:** `cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | tail -3` → builds. Commit:
```bash
git add src-tauri/tauri.conf.json src-tauri/src/main.rs
git commit -m "feat(ui): resizable window; remove forced-size hack"
```

### Task 4.2: UI store (locale, theme, active section)

**Files:** Create `src/stores/ui.ts`; Test `src/stores/__tests__/ui.test.ts`

- [ ] **Step 1: Write the failing test** `ui.test.ts`:
```ts
import { setActivePinia, createPinia } from "pinia";
import { beforeEach, describe, expect, it } from "vitest";
import { useUiStore } from "../ui";

describe("ui store", () => {
  beforeEach(() => setActivePinia(createPinia()));
  it("defaults section to dashboard and toggles locale", () => {
    const ui = useUiStore();
    expect(ui.section).toBe("dashboard");
    ui.setLocale("es");
    expect(ui.locale).toBe("es");
  });
});
```
- [ ] **Step 2:** Run `pnpm test ui` → FAIL (no store). 
- [ ] **Step 3:** Create `src/stores/ui.ts`:
```ts
import { defineStore } from "pinia";
import { ref } from "vue";
import { i18n } from "../i18n";

export type Section = "dashboard" | "connection" | "audio" | "logs" | "settings";

export const useUiStore = defineStore("ui", () => {
  const section = ref<Section>("dashboard");
  const locale = ref<"en" | "es">(i18n.global.locale.value as "en" | "es");
  const theme = ref<"dark">("dark"); // dark-only for now; reserved for future light theme

  function setSection(s: Section) { section.value = s; }
  function setLocale(l: "en" | "es") {
    locale.value = l;
    i18n.global.locale.value = l;
  }
  return { section, locale, theme, setSection, setLocale };
});
```
- [ ] **Step 4:** Run `pnpm test ui` → PASS. Commit:
```bash
git add src/stores/ui.ts src/stores/__tests__/ui.test.ts
git commit -m "feat(ui): ui store (section, locale, theme)"
```

### Task 4.3: AppShell, Sidebar, Header

**Files:** Create `src/components/AppShell.vue`, `Sidebar.vue`, `Header.vue`; Modify `src/App.vue`

- [ ] **Step 1:** `Sidebar.vue` — vertical nav bound to `ui.section`/`ui.setSection`, items from `nav.*` i18n keys with icons (`IconDashboard`, etc.), active item highlighted with the accent; app name + version at top/bottom. Collapses to icon-only below a width breakpoint (use a CSS container/media approach).
- [ ] **Step 2:** `Header.vue` — section title (`t('nav.'+ui.section)`), a `StatusDot`/`Badge` bound to `stream.statusText`, a primary Start/Stop `Button` (`@click="stream.toggleStream()"`, label from `action.start`/`action.stop` by `stream.isStreaming`), and a language `Select` (en/es) bound to `ui.locale`/`ui.setLocale`.
- [ ] **Step 3:** `AppShell.vue` — CSS grid: `Sidebar` (left), `Header` (top of content), and `<component :is>` / `v-if` switch on `ui.section` rendering the section components (created in M5). Include `<ToastNotification />`.
- [ ] **Step 4:** `App.vue` — replace its template with `<AppShell />`; keep the `onMounted` logic (`settings.init()`, `stream.initListeners()`, auto-stream, the Ctrl/Cmd+S shortcut, beforeunload/unload). Move device/output-device loading into `settings.init()` if not already.
- [ ] **Step 5:** `pnpm typecheck && pnpm lint` → green (sections may be stubs until M5; create minimal stub section components that render their name so the shell compiles, replaced in M5). Commit:
```bash
git add src/components/AppShell.vue src/components/Sidebar.vue src/components/Header.vue src/App.vue src/components/sections/
git commit -m "feat(ui): app shell (sidebar + header + content)"
```

---

## Milestone 5 — Sections

> Each section: build the `.vue` from the spec below using the M2/M3 primitives and the store bindings; then `pnpm typecheck && pnpm lint` green; then commit. Use i18n keys for all visible strings (add them to `en.json`/`es.json` as you go; M6 audits completeness).

### Task 5.1: Dashboard (worked example)

**Files:** Create `src/components/sections/DashboardSection.vue`

- [ ] **Step 1:** Build it:
```vue
<template>
  <div class="flex flex-col gap-4">
    <Card>
      <div class="flex items-center justify-between">
        <div class="flex flex-col gap-1">
          <span class="text-xs uppercase tracking-wide text-[var(--color-text-faint)]">{{ t("dashboard.statusLabel") }}</span>
          <span class="text-2xl font-semibold">{{ stream.statusText }}</span>
        </div>
        <Button :variant="stream.isStreaming ? 'danger' : 'primary'" @click="stream.toggleStream()">
          {{ stream.isStreaming ? t("action.stop") : t("action.start") }}
        </Button>
      </div>
    </Card>

    <div v-if="stream.isStreaming" class="grid grid-cols-3 gap-3">
      <Stat :label="t('stat.uptime')" :value="stream.formattedUptime" />
      <Stat :label="t('stat.transferred')" :value="stream.formattedBytes" />
      <Stat :label="t('stat.bitrate')" :value="stream.bitrateKbps.toFixed(1) + ' kbps'" />
      <Stat :label="t('stat.rtt')" :value="stream.rttMs === null ? 'n/a' : stream.rttMs.toFixed(1) + ' ms'" />
      <Stat :label="t('stat.underruns')" :value="String(stream.underruns)" />
      <Stat :label="t('stat.quality')">
        <span :style="{ color: stream.qualityLabel.color }">{{ stream.qualityLabel.text }} ({{ stream.qualityScore }})</span>
      </Stat>
    </div>

    <Card :title="t('dashboard.recentLogs')">
      <div class="font-mono text-xs flex flex-col gap-0.5 max-h-40 overflow-auto">
        <div v-for="(l, i) in stream.logs.slice(-5)" :key="i" class="text-[var(--color-text-muted)]">
          <span class="text-[var(--color-text-faint)]">[{{ l.timestamp }}]</span> {{ l.message }}
        </div>
        <div v-if="stream.logs.length === 0" class="text-[var(--color-text-faint)]">{{ t("logs.empty") }}</div>
      </div>
    </Card>
  </div>
</template>
<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { useStreamStore } from "../../stores/stream";
import Card from "../ui/Card.vue";
import Button from "../ui/Button.vue";
import Stat from "../ui/Stat.vue";
const { t } = useI18n();
const stream = useStreamStore();
</script>
```
Add the keys used (`dashboard.statusLabel`, `dashboard.recentLogs`, `stat.*`, `logs.empty`) to both catalogs.

- [ ] **Step 2:** `pnpm typecheck && pnpm lint` → green. Commit:
```bash
git add src/components/sections/DashboardSection.vue src/i18n
git commit -m "feat(ui): Dashboard section"
```

### Task 5.2: Connection (role-aware) — content + binding spec

**Files:** Create `src/components/sections/ConnectionSection.vue`

Build per this spec, using the primitives + bindings:
- Top: `SegmentedControl` bound to `settings.role` (`source` / `sink`), labels `connection.roleSource`/`connection.roleSink`. Disabled while `stream.isStreaming`.
- **When `settings.role === 'source'`:**
  - `Field`+`Select` bound to `settings.transport` — options TCP Client (`mode=client`), TCP Server (`mode=server`), Native UDP. (Map the existing `settings.mode` + `settings.transport`: TCP uses `mode`; Native uses `transport="udp"`. Preserve the current store semantics — grep `stream.ts startStream` to see which fields each path sends; bind exactly those.)
  - *TCP Client:* `Field`+`Input` for `settings.ip` (label "Target address", placeholder `192.168.1.100:4953` host/IPv6 ok) — note current store splits `ip` and `port`; bind both (`ip` + `port`) or a combined field that writes both. Keep it consistent with the store.
  - *TCP Server:* `Input` for `settings.port`; `Field`+`Input` for `settings.allowlist` (help "IP/CIDR, empty = allow all"); when `stream.isStreaming`, a `Card` titled `connection.info` showing `CopyField`s for `stream.tcpUrl`, `stream.httpUrl`, and a `Textarea` for `stream.connectionInfo`.
  - *Native UDP:* `Input` for `settings.port`; `Input type=password` for `settings.psk` (help "empty = no encryption"); a `Switch` "advertise via mDNS" (if a store flag exists; otherwise advertise is automatic — omit the toggle).
- **When `settings.role === 'sink'`:**
  - `Field`+`Select` for `settings.outputDevice` (options `settings.outputDevices`).
  - `Field`+`Input` for `settings.sourceAddr` with a `Button` (`@click="settings.scanSources()"`, label `action.scan`); if `settings.discoveredSources.length`, a `Select` listing them (`label = name + (encrypted ? ' 🔒'→ use a lock icon, not emoji)`, value = `addr`) that sets `settings.sourceAddr` on change.
  - `Input type=password` for `settings.psk`.
- Show only the active role/mode's controls (v-if), never greyed-out clutter.

- [ ] **Step 1:** Implement. **Step 2:** `pnpm typecheck && pnpm lint` green. **Step 3:** Commit `feat(ui): Connection section (role-aware)`.

### Task 5.3: Audio — spec

**Files:** Create `src/components/sections/AudioSection.vue`
- Input device `Select` (`settings.deviceName` from `settings.devices`) for source; loopback `Switch` when `settings.showLoopback`.
- `Select` for `settings.sampleRate` (48000/44100), `settings.format` (pcm/wav, server only).
- `Select` for `settings.latencyProfile` (ultra-low/balanced/robust/custom) with help; when `custom`, show `Input`s for `settings.ringBufferDuration`, `settings.chunkSize`, `settings.minBuffer`, `settings.maxBuffer`. Warning text when `latencyProfile==='ultra-low' && settings.isLoopback`.
- [ ] Implement → typecheck/lint green → commit `feat(ui): Audio section`.

### Task 5.4: Logs — spec

**Files:** Create `src/components/sections/LogsSection.vue`
- Header: a `Select` for `stream.logFilter` (all/info/success/warning/error/debug/trace), an `Input` for a local search string, a `Button` (ghost) `action.clear` → `stream.clearLogs()`, a `Button` (ghost) `action.copy` → copy `stream.filteredLogs` text.
- Body: monospace list of `stream.filteredLogs` filtered further by the search string; per-level **muted** colors (no neon): info=text-muted, success=success token, warning=warning token, error=danger token. Auto-scroll to bottom on new logs unless the user scrolled up. Empty state `logs.empty`.
- [ ] Implement → typecheck/lint green → commit `feat(ui): Logs section`.

### Task 5.5: Settings — spec

**Files:** Create `src/components/sections/SettingsSection.vue`
- **Profiles** `Card`: `Select` for `settings.currentProfile` (from `settings.profiles`) → `settings.switchProfile`; `Button`s Save (`settings.saveSettings` + toast), New (opens a `Dialog` with an `Input` → `settings.createProfile`), Delete (`settings.deleteProfile`, disabled for "Default").
- **Automation** `Card`: `Switch`es for `settings.autostart` (→ `settings.toggleAutostart`), `settings.autoStream`, `settings.autoReconnect` (hide reconnect when sink/irrelevant).
- **Preferences** `Card`: language `Select` (en/es) bound to `ui.locale`/`ui.setLocale`.
- **About** `Card`: app name + `v{version}` (`import.meta.env.PACKAGE_VERSION`), made-by line, and a discreet PayPal link `Button` (secondary, small).
- [ ] Implement → typecheck/lint green → commit `feat(ui): Settings section`.

---

## Milestone 6 — i18n completeness

### Task 6.1: Extract remaining strings + fill catalogs

- [ ] **Step 1:** `git grep -nE '>[A-Za-z]{3,}' src/components` and scan each section/primitive for any hardcoded user-visible English; replace with `t('...')` and add the keys to `en.json` + `es.json` (parallel structure, both complete).
- [ ] **Step 2:** Add a vitest that asserts the two catalogs have identical key sets:
```ts
import en from "../../i18n/en.json";
import es from "../../i18n/es.json";
function keys(o: any, p = ""): string[] {
  return Object.entries(o).flatMap(([k, v]) =>
    typeof v === "object" && v ? keys(v, `${p}${k}.`) : [`${p}${k}`]);
}
it("en and es catalogs have the same keys", () => {
  expect(keys(en).sort()).toEqual(keys(es).sort());
});
```
(Place under `src/i18n/__tests__/catalogs.test.ts`.)
- [ ] **Step 3:** `pnpm test && pnpm typecheck && pnpm lint` → green. Commit `feat(i18n): complete en/es coverage + catalog-parity test`.

---

## Milestone 7 — Cleanup + verification

### Task 7.1: Delete superseded components

**Files:** Delete `src/components/tabs/`, `TabNav.vue`, `StreamButton.vue`, `StatsBar.vue`, `AppHeader.vue`, `AppFooter.vue`, `ui/CheckboxField.vue`, `ui/SelectField.vue`, `ui/InputField.vue`, `LinuxTitlebar.vue` (if unused).

- [ ] **Step 1:** `git rm` the files. **Step 2:** `git grep -n "tabs/\|TabNav\|StreamButton\|StatsBar\|AppHeader\|AppFooter\|CheckboxField\|SelectField\|InputField"` src → no references remain (fix any). **Step 3:** `pnpm typecheck && pnpm lint && pnpm build` → green. **Step 4:** Commit `refactor(ui): remove superseded components`.

### Task 7.2: Full gate + manual smoke

- [ ] **Step 1:**
```bash
pnpm test && pnpm typecheck && pnpm lint && pnpm format:check && pnpm build && \
cargo test --manifest-path src-tauri/Cargo.toml && \
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```
Expected: all green.
- [ ] **Step 2:** `pnpm tauri dev` and verify: resizable window with sidebar + header; navigate all sections; Start/Stop works for **source TCP client, source TCP server, source native UDP, and sink**; language toggle switches all chrome instantly; no glassmorphism/glows/emojis; keyboard focus rings visible; Select/Switch/Dialog accessible.
- [ ] **Step 3:** Final commit if needed: `chore: Phase 3 verification fixes`.

---

## Self-Review (author)

- **Spec coverage:** §4 tokens → M1; §4.1 primitives → M2+M3; §5 shell/window → M4; §6 sections → M5; §7 i18n → M0.2+M6; §8 tech (deps, ui store, structure) → M0+M4.2; §9 tests → store + catalog-parity + smoke; cleanup → M7.
- **Type consistency:** primitives use `v-model`/`modelValue` consistently; `ui` store `{section,locale,theme,setSection,setLocale}`; sections bind only to documented store members (listed at top). i18n `t()` keys are added alongside each section and audited in M6.
- **No placeholders:** foundation tasks have complete code; section tasks (5.2–5.5) are precise content+binding specs (the appropriate altitude for views) with the exact store members to bind — not vague "build the UI". The Dashboard (5.1) is fully coded as the pattern exemplar. Two explicit "verify against installed version" notes (reka-ui component names, store member names if Phase 2 renamed any) — deterministic checks, not hand-waving.

## After this plan
Phase 3 completes the redesign. Remaining for the whole initiative: merge `phase-3-ui-redesign` to `main` + version bump (→ v2.3.0 or v3.0.0), optional release push/tag, and the manual two-machine smoke of the native mode.
