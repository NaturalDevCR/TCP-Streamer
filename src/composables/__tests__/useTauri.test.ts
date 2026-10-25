import { beforeEach, describe, expect, it, vi } from "vitest";
import { i18n } from "../../i18n";
const native = vi.hoisted(() => ({
  isTauri: vi.fn(),
  invoke: vi.fn(),
  listen: vi.fn(),
  load: vi.fn(),
  enable: vi.fn(),
  disable: vi.fn(),
  isEnabled: vi.fn(),
  getCurrentWindow: vi.fn(),
}));
vi.mock("@tauri-apps/api/core", () => ({ isTauri: native.isTauri, invoke: native.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: native.listen }));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: native.getCurrentWindow }));
vi.mock("@tauri-apps/plugin-store", () => ({ Store: { load: native.load } }));
vi.mock("@tauri-apps/plugin-autostart", () => ({
  enable: native.enable,
  disable: native.disable,
  isEnabled: native.isEnabled,
}));
import {
  invoke,
  listen,
  getStore,
  enable,
  disable,
  isEnabled,
  getCurrentWindow,
} from "../useTauri";

beforeEach(() => {
  vi.clearAllMocks();
  i18n.global.locale.value = "en";
});
describe("native capability boundary", () => {
  it("rejects native operations with a localized explanation before accessing missing IPC", async () => {
    native.isTauri.mockReturnValue(false);
    const expected = i18n.global.t("runtime.desktopRequired");
    for (const action of [
      () => invoke("start_stream"),
      () => listen("stream-state", () => {}),
      getStore,
      enable,
      disable,
      isEnabled,
    ]) {
      await expect(action()).rejects.toThrow(expected);
    }
    expect(getCurrentWindow).toThrow(expected);
    for (const [name, method] of Object.entries(native))
      if (name !== "isTauri") expect(method).not.toHaveBeenCalled();
    i18n.global.locale.value = "es";
    await expect(invoke("start_stream")).rejects.toThrow(
      "Esta acción requiere la app de escritorio",
    );
  });

  it("forwards desktop commands and events, propagates native failures, and retries failed store opens", async () => {
    native.isTauri.mockReturnValue(true);
    native.invoke.mockResolvedValue({ state: "waiting" });
    const options = { deviceName: "mic", transport: "udp" };
    await expect(invoke("start_stream", options)).resolves.toEqual({ state: "waiting" });
    expect(native.invoke).toHaveBeenCalledWith("start_stream", options);
    const callback = vi.fn(),
      unlisten = vi.fn();
    native.listen.mockResolvedValue(unlisten);
    await expect(listen("stream-state", callback)).resolves.toBe(unlisten);
    expect(native.listen).toHaveBeenCalledWith("stream-state", callback);
    native.invoke.mockRejectedValue(new Error("device disconnected"));
    await expect(invoke("start_stream")).rejects.toThrow("device disconnected");
    const store = { get: vi.fn(), set: vi.fn(), save: vi.fn() };
    native.load.mockRejectedValueOnce(new Error("permission denied")).mockResolvedValueOnce(store);
    await expect(getStore()).rejects.toThrow("permission denied");
    await expect(getStore()).resolves.toBe(store);
    expect(native.load).toHaveBeenCalledTimes(2);
  });
});
