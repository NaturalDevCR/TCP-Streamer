# Configurable Option Tooltips Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add accessible, bilingual tooltip icons to every configurable option in Connection, Audio, and Settings, then publish and verify release v2.3.0.

**Architecture:** A new `SettingLabel.vue` component owns the visible label, native button trigger, localized accessible name, and existing Reka UI tooltip. `Field.vue` delegates label rendering to it, while switch rows use it directly. Section components provide localized tooltip copy from the existing i18n catalogs.

**Tech Stack:** Vue 3, TypeScript, Reka UI, vue-i18n, Vitest, Tauri 2, GitHub Actions.

---

### Task 1: Accessible setting label

**Files:**

- Create: `src/components/ui/SettingLabel.vue`
- Create: `src/components/ui/__tests__/SettingLabel.test.ts`
- Modify: `src/components/ui/Field.vue`
- Create: `src/components/ui/__tests__/Field.test.ts`
- Modify: `src/i18n/en.json`
- Modify: `src/i18n/es.json`

- [ ] **Step 1: Write failing tests**

Add tests that mount `SettingLabel` with `Tooltip` stubbed and assert that:

```ts
expect(wrapper.get("label").attributes("for")).toBe("sample-rate");
expect(wrapper.get("button").attributes("type")).toBe("button");
expect(wrapper.get("button").attributes("aria-label")).toBe(
  "More information about Output Sample Rate",
);
expect(wrapper.get('[data-test="tooltip-content"]').text()).toBe("Controls the wire rate.");
```

Add a second test without `tooltip`:

```ts
expect(wrapper.find("button").exists()).toBe(false);
```

Add `Field` tests asserting it renders the setting label and does not render a
tooltip trigger when no tooltip text is supplied.

- [ ] **Step 2: Run tests and verify RED**

Run:

```bash
pnpm test src/components/ui/__tests__/SettingLabel.test.ts src/components/ui/__tests__/Field.test.ts
```

Expected: FAIL because `SettingLabel.vue` and the new `Field` props do not exist.

- [ ] **Step 3: Implement the reusable label**

Create `SettingLabel.vue` with this public API:

```ts
const props = defineProps<{
  label: string;
  forId?: string;
  tooltip?: string;
}>();
```

Render a `<label>` when `forId` is present and a `<span>` otherwise. When
`tooltip` is present, render:

```vue
<Tooltip>
  <template #trigger>
    <button
      type="button"
      class="focus-ring inline-flex h-4 w-4 items-center justify-center rounded-full"
      :aria-label="t('tooltip.moreInfo', { option: label })"
    >
      <span aria-hidden="true">i</span>
    </button>
  </template>
  <span class="block max-w-64 leading-relaxed">{{ tooltip }}</span>
</Tooltip>
```

Add the shared catalog key:

```json
"tooltip": {
  "moreInfo": "More information about {option}"
}
```

and:

```json
"tooltip": {
  "moreInfo": "Más información sobre {option}"
}
```

Update `Field.vue` to accept `tooltip?: string` and render `SettingLabel`.

- [ ] **Step 4: Run tests and verify GREEN**

Run:

```bash
pnpm test src/components/ui/__tests__/SettingLabel.test.ts src/components/ui/__tests__/Field.test.ts src/i18n/__tests__/catalogs.test.ts
```

Expected: all tests PASS.

### Task 2: Wire every configurable option

**Files:**

- Modify: `src/components/sections/ConnectionSection.vue`
- Modify: `src/components/sections/AudioSection.vue`
- Modify: `src/components/sections/SettingsSection.vue`
- Modify: `src/i18n/en.json`
- Modify: `src/i18n/es.json`

- [ ] **Step 1: Add bilingual tooltip copy**

Add matching `help` objects under `connection`, `audio`, and `settings`.

Connection keys:

```text
role, inputDevice, loopback, transport, mode, psk, targetAddress, port,
allowlist, outputDevice, sourceAddress, discoveredSources
```

Audio keys:

```text
inputDevice, sampleRate, bufferSize, format, loopback, latencyProfile,
ringBufferDuration, chunkSize, adaptiveBuffer, minBuffer, maxBuffer
```

Settings keys:

```text
profile, autoStart, autoStream, autoReconnect, language
```

Each string must explain the effect and practical use of its control, not merely
repeat the visible label.

- [ ] **Step 2: Attach tooltips to fields**

For every `Field` in Connection and Audio that changes settings, pass its help
text:

```vue
<Field
  :label="t('audio.sampleRate')"
  :tooltip="t('audio.help.sampleRate')"
>
```

Translate the remaining hardcoded configurable labels with:

```json
"transport": "Transport",
"mode": "Mode"
```

and:

```json
"transport": "Transporte",
"mode": "Modo"
```

Replace permanent `help` text for allowlist, PSK, and sample rate with tooltip
text. Keep operational warnings visible.

- [ ] **Step 3: Attach tooltips to switches**

Import `SettingLabel` into the three section components and replace switch-row
label blocks with:

```vue
<SettingLabel :label="t('settings.autoStart')" :tooltip="t('settings.help.autoStart')" />
```

Apply the same pattern to loopback, adaptive buffer, auto-stream, and
auto-reconnect.

- [ ] **Step 4: Verify catalogs and frontend**

Run:

```bash
pnpm test
pnpm lint
pnpm format:check
pnpm typecheck
pnpm build
```

Expected: all commands exit 0.

### Task 3: Visual and native verification

**Files:**

- Modify: `.github/workflows/build.yml`
- Modify: `.github/workflows/release.yml`
- Modify: `package.json`
- Modify: `pnpm-lock.yaml`
- Modify only files required by failures discovered during verification.

- [ ] **Step 1: Install Linux native dependencies before Rust CI checks**

The current `lint-and-test` jobs run `cargo test` on Ubuntu without the native
GTK, WebKit, app-indicator, SVG, ALSA, and SSL development packages required by
Tauri and CPAL. Add this step before Rust setup in both workflows:

```yaml
- name: Install native dependencies
  run: |
    sudo apt-get update
    sudo apt-get install -y libgtk-3-dev libwebkit2gtk-4.1-dev \
      libayatana-appindicator3-dev librsvg2-dev libasound2-dev libssl-dev
```

- [ ] **Step 2: Inspect the UI**

Run the development server and use the in-app browser to inspect Connection,
Audio, and Settings. Verify:

```text
- Every configurable control has an information icon.
- Hover and keyboard focus open the localized tooltip.
- Tooltip icons do not change the associated setting.
- Visible warnings remain visible.
- English and Spanish layouts do not clip.
```

- [ ] **Step 3: Run full native checks**

Run:

```bash
pnpm test
pnpm lint
pnpm format:check
pnpm typecheck
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml --check
pnpm tauri build
```

Expected: every command exits 0.

- [ ] **Step 4: Align Tauri JavaScript and Rust versions**

The top-level JavaScript API must use the same major/minor release line as the
Rust `tauri` crate. Pin the direct dependency to the available 2.9 release:

```json
"@tauri-apps/api": "2.9.1"
```

Run `pnpm install --lockfile-only`, reinstall with `pnpm install
--frozen-lockfile`, and rerun `pnpm tauri build`.

- [ ] **Step 5: Commit the implementation**

Run:

```bash
git add docs/superpowers/plans/2026-06-12-tooltips-opciones-configurables.md \
  .github/workflows/build.yml \
  .github/workflows/release.yml \
  package.json \
  pnpm-lock.yaml \
  src/components/ui/SettingLabel.vue \
  src/components/ui/Field.vue \
  src/components/ui/__tests__/SettingLabel.test.ts \
  src/components/ui/__tests__/Field.test.ts \
  src/components/sections/__tests__/configHelp.test.ts \
  src/components/sections/ConnectionSection.vue \
  src/components/sections/AudioSection.vue \
  src/components/sections/SettingsSection.vue \
  src/i18n/__tests__/catalogs.test.ts \
  src/i18n/en.json \
  src/i18n/es.json
git commit -m "feat(ui): explain configurable options with tooltips"
```

### Task 4: Publish v2.3.0 and monitor CI

**Files:**

- Modify only files required to fix a reproducible CI or release failure.

- [ ] **Step 1: Push main**

Run:

```bash
git push origin main
```

Wait for the Build workflow for the pushed SHA. Inspect failed job logs with
`gh run view`; implement, verify, commit, and push focused fixes until Build is
green.

- [ ] **Step 2: Publish the release tag**

After Build is green:

```bash
git tag -a v2.3.0 -m "TCP Streamer v2.3.0"
git push origin v2.3.0
```

- [ ] **Step 3: Verify all release platforms**

Watch the Release workflow until completion and confirm successful jobs for:

```text
macos-latest
ubuntu-22.04
windows-latest
```

If a job fails, inspect its logs, apply a focused fix on `main`, move the local
and remote `v2.3.0` tag only if no release was successfully published, and
rerun until the release is green and contains artifacts for all three
platforms.

- [ ] **Step 4: Confirm final release**

Run:

```bash
gh release view v2.3.0 --json url,isDraft,isPrerelease,assets
gh run list --limit 10
git status -sb
```

Expected: published non-prerelease release, platform assets present, Build and
Release green, and clean local `main` synchronized with `origin/main`.
