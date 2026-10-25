import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { mount, flushPromises, type VueWrapper } from "@vue/test-utils";
import { i18n } from "../../i18n";
import App from "../../App.vue";
import { useSettingsStore } from "../../stores/settings";
import { useStreamStore } from "../../stores/stream";

const native = vi.hoisted(() => ({
  invoke: vi.fn(),
  listen: vi.fn(),
  load: vi.fn(),
  isEnabled: vi.fn(),
}));
vi.mock("@tauri-apps/api/core", () => ({ isTauri: () => false, invoke: native.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: native.listen }));
vi.mock("@tauri-apps/plugin-store", () => ({ Store: { load: native.load } }));
vi.mock("@tauri-apps/plugin-autostart", () => ({ isEnabled: native.isEnabled }));

let wrapper: VueWrapper;
beforeEach(() => {
  vi.clearAllMocks();
  const pinia = createPinia();
  setActivePinia(pinia);
  i18n.global.locale.value = "en";
  wrapper = mount(App, { global: { plugins: [pinia, i18n] } });
});
afterEach(() => {
  wrapper.unmount();
  document.body.innerHTML = "";
});
function expectNoNativeCalls() {
  for (const method of Object.values(native)) expect(method).not.toHaveBeenCalled();
}

describe("browser preview with no Tauri runtime", () => {
  it("mounts the real application without IPC errors or native calls", async () => {
    await flushPromises();
    expect(wrapper.text()).toContain("Browser preview");
    expect(wrapper.text()).not.toContain("TypeError");
    expect(wrapper.text()).not.toContain("undefined");
    const settings = useSettingsStore();
    expect(settings.nativeAvailable).toBe(false);
    expect(settings.profiles).toEqual({ Default: {} });
    expect(settings.isDirty).toBe(false);
    expect(settings.persistenceError).toBe("");
    expect(settings.devicesError).toBe("");
    expect(settings.outputDevicesError).toBe("");
    expect(useStreamStore().sessionState).toBe("stopped");
    expect(wrapper.get(".workspace-footer b").text()).toBe("—");
    expect(wrapper.get(".header-actions button.ui-button").attributes("disabled")).toBeDefined();
    expectNoNativeCalls();
  });

  it("allows UI configuration while disabling native actions and their shortcuts", async () => {
    await flushPromises();
    await wrapper.get('nav button[aria-label="Connection"]').trigger("click");
    const buttons = wrapper.findAll("main button");
    expect(
      buttons.find((button) => button.text() === "Refresh devices")?.attributes("disabled"),
    ).toBeDefined();
    expect(
      buttons.find((button) => button.text() === "Run checks")?.attributes("disabled"),
    ).toBeDefined();
    await wrapper.get('[role="radio"][aria-checked="false"]').trigger("click");
    expect(useSettingsStore().role).toBe("sink");
    expect(
      wrapper
        .findAll("main button")
        .find((button) => button.text() === "Scan")
        ?.attributes("disabled"),
    ).toBeDefined();
    document.dispatchEvent(
      new KeyboardEvent("keydown", { key: "s", ctrlKey: true, cancelable: true }),
    );
    document.dispatchEvent(
      new KeyboardEvent("keydown", { key: "Enter", ctrlKey: true, cancelable: true }),
    );
    await flushPromises();
    expect(useStreamStore().isStreaming).toBe(false);
    expect(useSettingsStore().persistenceError).toBe("");
    expectNoNativeCalls();
  });

  it("clears previous native startup errors when reinitializing Settings in the browser", async () => {
    await flushPromises();
    const settings = useSettingsStore();
    const previousError = "TypeError: Cannot read properties of undefined (reading 'invoke')";
    settings.persistenceError = previousError;
    settings.devicesError = previousError;
    settings.outputDevicesError = previousError;
    settings.scanError = previousError;
    settings.preflightError = previousError;
    await settings.init();
    await wrapper.get('nav button[aria-label="Settings"]').trigger("click");
    expect(wrapper.get("h1").text()).toBe("Make it yours");
    expect(wrapper.text()).toContain("Browser preview");
    expect(wrapper.text()).not.toContain(previousError);
    expect(wrapper.find('[role="alert"]').exists()).toBe(false);
    expect(settings.persistenceError).toBe("");
    expect(settings.devicesError).toBe("");
    expect(settings.outputDevicesError).toBe("");
    expect(settings.scanError).toBe("");
    expect(settings.preflightError).toBe("");
    expect(settings.currentProfile).toBe("Default");
    expect(settings.isDirty).toBe(false);
    expectNoNativeCalls();
  });
});
