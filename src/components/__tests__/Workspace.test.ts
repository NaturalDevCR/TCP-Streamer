import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { mount, type VueWrapper } from "@vue/test-utils";
import { nextTick } from "vue";
import { i18n } from "../../i18n";
import { useUiStore } from "../../stores/ui";
import { useSettingsStore } from "../../stores/settings";
import { useStreamStore } from "../../stores/stream";
import AppShell from "../AppShell.vue";

vi.mock("../../composables/useTauri", () => ({
  isNativeRuntime: () => true,
  invoke: vi.fn().mockResolvedValue([]),
  listen: vi.fn().mockResolvedValue(() => {}),
}));

let wrapper: VueWrapper;
beforeEach(() => {
  const pinia = createPinia();
  setActivePinia(pinia);
  i18n.global.locale.value = "en";
  wrapper = mount(AppShell, { global: { plugins: [pinia, i18n] } });
});
afterEach(() => {
  wrapper.unmount();
  document.body.innerHTML = "";
});

async function openSection(section: "dashboard" | "connection" | "audio" | "logs" | "settings") {
  await wrapper.get(`nav button[aria-label="${i18n.global.t(`nav.${section}`)}"]`).trigger("click");
}

describe("redesigned workspace", () => {
  it("navigates to every workspace and translates its heading and navigation", async () => {
    for (const section of ["dashboard", "connection", "audio", "logs", "settings"] as const) {
      await openSection(section);
      expect(wrapper.get("h1").text()).toBe(i18n.global.t(`workspace.titles.${section}`));
      expect(wrapper.findAll('nav [aria-current="page"]')).toHaveLength(1);
    }
    useUiStore().setLocale("es");
    await nextTick();
    expect(wrapper.get("h1").text()).toBe("A tu manera");
    expect(wrapper.get('nav [aria-current="page"]').text()).toContain("Ajustes");
    expect(wrapper.text()).not.toContain("workspace.");
  });

  it("keeps the language selector available during an active session", async () => {
    useSettingsStore().configurationLocked = true;
    await openSection("settings");
    const selector = wrapper.get('header.workspace-header button[aria-label="Language"]');
    expect(selector.element.closest("fieldset[disabled]")).toBeNull();
    expect(selector.attributes("disabled")).toBeUndefined();
  });

  it("shows missing metrics instead of presenting idle values as measured data", () => {
    const stream = useStreamStore();
    stream.statsAvailable = true;
    stream.bitrateKbps = 1536;
    expect(wrapper.findAll(".stat-value").map((el) => el.text())).toEqual(["—", "—", "—", "—"]);
    expect(wrapper.text()).toContain("Waiting for a session");
  });

  it("uses the active session route and actual metrics when streaming", async () => {
    const stream = useStreamStore();
    const settings = useSettingsStore();
    settings.deviceName = "Next input";
    settings.ip = "next-host";
    stream.isStreaming = true;
    stream.activeSettings = {
      role: "source",
      transport: "tcp",
      mode: "client",
      device: "Live input",
      ip: "::1",
      port: 4953,
    };
    stream.statsAvailable = true;
    stream.bitrateKbps = 1536;
    await nextTick();
    expect(wrapper.get(".signal-path").text()).toContain("Live input");
    expect(wrapper.get(".signal-path").text()).toContain("[::1]:4953");
    expect(wrapper.get(".signal-path").text()).not.toContain("next-host");
    expect(wrapper.text()).toContain("1536.0 kbps");
  });

  it("keeps running TCP server addresses copyable outside the locked configuration", async () => {
    const settings = useSettingsStore();
    settings.mode = "server";
    settings.configurationLocked = true;
    useStreamStore().tcpUrl = "tcp://127.0.0.1:1704";
    await openSection("connection");
    const copyButtons = wrapper.findAll('button[aria-label="Copy"]');
    expect(copyButtons).toHaveLength(2);
    for (const button of copyButtons) {
      expect(button.element.closest("fieldset[disabled]")).toBeNull();
      expect(button.attributes("disabled")).toBeUndefined();
    }
    expect(wrapper.get<HTMLInputElement>("input[readonly]").element.value).toBe(
      "tcp://127.0.0.1:1704",
    );
    expect(wrapper.get('button[role="radio"]').attributes("disabled")).toBeDefined();
  });

  it("preserves receiver discovery, output selection, and shared-secret controls", async () => {
    await openSection("connection");
    await wrapper.get('[role="radio"][aria-checked="false"]').trigger("click");
    expect(useSettingsStore().role).toBe("sink");
    expect(wrapper.find('input[aria-label="Source Address"]').exists()).toBe(true);
    expect(wrapper.find('button[aria-label="Output Device"]').exists()).toBe(true);
    expect(wrapper.find('input[type="password"]').exists()).toBe(true);
    expect(
      wrapper.find('[role="switch"][aria-label="Remember shared secret locally"]').exists(),
    ).toBe(true);
    expect(wrapper.text()).toContain("Scan");
    expect(wrapper.text()).not.toContain("Client (Send Audio to IP)");
  });

  it("exposes fixed and custom latency settings through keyboard-accessible presets", async () => {
    await openSection("audio");
    const presets = wrapper.get(".latency-presets");
    await presets.findAll('[role="radio"]')[3].trigger("click");
    expect(useSettingsStore().latencyProfile).toBe("broadcast");
    expect(wrapper.find('input[aria-label="Latency target (ms)"]').exists()).toBe(true);
    await presets.findAll('[role="radio"]')[3].trigger("keydown", { key: "ArrowRight" });
    expect(useSettingsStore().latencyProfile).toBe("custom");
    expect(wrapper.find('input[aria-label="Min Buffer (ms)"]').exists()).toBe(true);
    expect(wrapper.find('input[aria-label="Max Buffer (ms)"]').exists()).toBe(true);
    expect(presets.findAll('[tabindex="0"]')).toHaveLength(1);
  });

  it("filters events by search without hiding the copy and export controls", async () => {
    const stream = useStreamStore();
    stream.logs = [
      { timestamp: "12:00:00", level: "info", message: "Connected to receiver", visible: true },
      { timestamp: "12:00:01", level: "warning", message: "Buffer adjusted", visible: true },
    ];
    await openSection("logs");
    expect(wrapper.findAll(".log-row")).toHaveLength(2);
    await wrapper.get('input[aria-label="Search logs..."]').setValue("buffer");
    expect(wrapper.findAll(".log-row")).toHaveLength(1);
    expect(wrapper.get(".log-row").text()).toContain("Buffer adjusted");
    expect(wrapper.find('button[aria-label="Copy"]').exists()).toBe(true);
    expect(wrapper.text()).toContain("Export sanitized");
  });
});
