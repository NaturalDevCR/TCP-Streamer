import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
const invoke = vi.hoisted(() => vi.fn());
vi.mock("../../composables/useTauri", () => ({
  invoke,
  getStore: vi.fn(),
  enable: vi.fn(),
  disable: vi.fn(),
  isEnabled: vi.fn(),
}));
import { useSettingsStore } from "../settings";
beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
});
describe("preflight configuration", () => {
  it("sends selected sink configuration without starting audio and invalidates edited results", async () => {
    invoke.mockResolvedValue({
      ready: true,
      stages: [{ name: "device", status: "ok", detail: "Device available" }],
    });
    const store = useSettingsStore();
    store.role = "sink";
    store.outputDevice = "speaker";
    store.sourceAddr = "[::1]:1704";
    await store.preflight();
    expect(invoke).toHaveBeenCalledWith("preflight_stream", {
      config: expect.objectContaining({
        role: "sink",
        device: "speaker",
        sourceAddr: "[::1]:1704",
      }),
    });
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(store.preflightResult?.ready).toBe(true);
    expect(store.preflightStale).toBe(false);
    store.sourceAddr = "changed:1704";
    expect(store.preflightStale).toBe(true);
  });
  it("blocks repeated checks and active-session port probes", async () => {
    let resolve!: (value: unknown) => void;
    invoke.mockImplementation(
      () =>
        new Promise((r) => {
          resolve = r;
        }),
    );
    const store = useSettingsStore();
    const check = store.preflight();
    await store.preflight();
    expect(invoke).toHaveBeenCalledTimes(1);
    resolve({ ready: false, stages: [] });
    await check;
    store.configurationLocked = true;
    await store.preflight();
    expect(invoke).toHaveBeenCalledTimes(1);
  });
});
