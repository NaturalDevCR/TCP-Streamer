import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
const bridge = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn(), getStore: vi.fn() }));
vi.mock("../../composables/useTauri", () => ({
  ...bridge,
  isEnabled: vi.fn(),
  enable: vi.fn(),
  disable: vi.fn(),
}));
import { useSettingsStore } from "../settings";
import { useStreamStore, type SessionStatus } from "../stream";
beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
  bridge.getStore.mockResolvedValue({
    get: vi.fn().mockResolvedValue(null),
    set: vi.fn(),
    save: vi.fn(),
  });
  bridge.invoke.mockResolvedValue({ state: "running", message: "", session_id: 1 });
});
describe("session coordination", () => {
  it("starts a clean UDP source without a hidden TCP destination and never enables a TCP server", async () => {
    const settings = useSettingsStore();
    settings.deviceName = "mic";
    settings.transport = "udp";
    settings.mode = "server";
    await useStreamStore().startStream();
    expect(bridge.invoke).toHaveBeenCalledWith(
      "start_stream",
      expect.objectContaining({ transport: "udp", isServer: false, ip: "" }),
    );
    expect(useStreamStore().tcpUrl).toBe("");
  });
  it("awaits acknowledgement and blocks duplicate starts", async () => {
    let resolve!: (s: SessionStatus) => void;
    bridge.invoke.mockImplementation(
      () =>
        new Promise((r) => {
          resolve = r;
        }),
    );
    const settings = useSettingsStore();
    settings.deviceName = "mic";
    settings.transport = "udp";
    const stream = useStreamStore();
    const pending = stream.startStream();
    await vi.waitFor(() => expect(bridge.invoke).toHaveBeenCalledTimes(1));
    expect(stream.pending).toBe(true);
    expect(stream.isStreaming).toBe(false);
    expect(settings.configurationLocked).toBe(true);
    await stream.startStream();
    expect(bridge.invoke).toHaveBeenCalledTimes(1);
    resolve({ state: "waiting", message: "", session_id: 1 });
    await pending;
    expect(stream.isStreaming).toBe(true);
    expect(stream.sessionState).toBe("waiting");
  });
  it("ignores previous sessions, clears counters and applies late engine failures", async () => {
    const settings = useSettingsStore();
    settings.deviceName = "mic";
    settings.transport = "udp";
    const stream = useStreamStore();
    stream.bytesSent = 456;
    stream.rttMs = 22;
    await stream.startStream();
    expect(stream.bytesSent).toBe(0);
    expect(stream.rttMs).toBeNull();
    stream.applySession({ state: "error", message: "old failure", session_id: 0 });
    expect(stream.isStreaming).toBe(true);
    stream.applySession({ state: "error", message: "device lost", session_id: 1 });
    expect(stream.isStreaming).toBe(false);
    expect(settings.configurationLocked).toBe(false);
  });
  it("shows persistence failure without attempting to start", async () => {
    bridge.getStore.mockRejectedValue(new Error("permission denied"));
    const settings = useSettingsStore();
    settings.deviceName = "mic";
    settings.transport = "udp";
    const stream = useStreamStore();
    await stream.startStream();
    expect(bridge.invoke).not.toHaveBeenCalled();
    expect(stream.statusText).toContain("permission denied");
    expect(stream.pending).toBe(false);
  });
});

describe("effective audio configuration", () => {
  it("uses native wire/device rates instead of requested settings and retains config across state-only events", () => {
    const settings = useSettingsStore();
    settings.sampleRate = 48000;
    const stream = useStreamStore();
    const effective = {
      role: "source",
      transport: "udp",
      profile: "balanced",
      sample_rate: 48000,
      channels: 2,
      device_sample_rate: 44100,
      device_channels: 1,
      ring_ms: 4000,
      target_ms: 200,
      chunk_frames: 512,
      capture_buffer_frames: 256,
      adaptive: true,
    };
    stream.applySession({ state: "running", message: "", session_id: 1, effective });
    expect(stream.effectiveConfig?.device_sample_rate).toBe(44100);
    stream.applySession({ state: "reconnecting", message: "", session_id: 1 });
    expect(stream.effectiveConfig).toEqual(effective);
    stream.applySession({ state: "starting", message: "", session_id: 2 });
    expect(stream.effectiveConfig).toBeNull();
  });
});

describe("startup cancellation", () => {
  it("allows Stop during Start and never resurrects from a late successful acknowledgement", async () => {
    let resolve!: (s: SessionStatus) => void;
    bridge.invoke.mockImplementation((cmd) =>
      cmd === "stop_stream"
        ? Promise.resolve({ state: "stopped", message: "", session_id: 1 })
        : new Promise((r) => {
            resolve = r;
          }),
    );
    const settings = useSettingsStore();
    settings.deviceName = "mic";
    settings.transport = "udp";
    const stream = useStreamStore();
    const start = stream.startStream();
    await vi.waitFor(() =>
      expect(bridge.invoke).toHaveBeenCalledWith("start_stream", expect.anything()),
    );
    await stream.toggleStream();
    expect(bridge.invoke).toHaveBeenCalledWith("stop_stream");
    resolve({ state: "running", message: "", session_id: 1 });
    await start;
    expect(stream.sessionState).toBe("stopped");
    expect(stream.isStreaming).toBe(false);
    expect(stream.pending).toBe(false);
  });
});

describe("session metrics", () => {
  it("rejects stale quality, preserves native counter units and hides quality while waiting", async () => {
    const listeners = new Map<string, (event: { payload: unknown }) => void>();
    const dispose = vi.fn();
    bridge.listen.mockImplementation((name, callback) => {
      listeners.set(name, callback);
      return Promise.resolve(dispose);
    });
    const stream = useStreamStore();
    await stream.initListeners();
    const quality = {
      score: 90,
      rtt_ms: null,
      rtt_var_ms: null,
      underruns: 12,
      dropped: 3,
      buffer_health: 0.25,
      counter_unit: "frames/frames",
    };
    listeners.get("quality-event")?.({ payload: { ...quality, session_id: 0 } });
    expect(stream.qualityAvailable).toBe(false);
    listeners.get("quality-event")?.({ payload: { ...quality, session_id: 1 } });
    expect(stream.qualityAvailable).toBe(true);
    expect(stream.qualityUnits).toBe("frames/frames");
    listeners.get("stream-state")?.({ payload: { state: "waiting", message: "", session_id: 1 } });
    expect(stream.qualityAvailable).toBe(false);
    listeners.get("quality-event")?.({ payload: { ...quality, score: 0, session_id: 1 } });
    expect(stream.qualityAvailable).toBe(false);
    expect(stream.toasts).toHaveLength(0);
    stream.disposeListeners();
    expect(dispose).toHaveBeenCalledTimes(5);
  });
});

describe("session status presentation", () => {
  it("distinguishes startup, waiting, reconnecting and failure from successful audio", () => {
    const stream = useStreamStore();
    expect(stream.statusTone).toBe("neutral");
    stream.applySession({ state: "starting", message: "", session_id: 1 });
    expect(stream.statusTone).toBe("warning");
    stream.applySession({ state: "waiting", message: "", session_id: 1 });
    expect(stream.statusTone).toBe("warning");
    stream.applySession({ state: "running", message: "", session_id: 1 });
    expect(stream.statusTone).toBe("success");
    stream.applySession({ state: "reconnecting", message: "", session_id: 1 });
    expect(stream.statusTone).toBe("warning");
    stream.applySession({ state: "error", message: "device failed", session_id: 1 });
    expect(stream.statusTone).toBe("danger");
    stream.applySession({ state: "stopped", message: "", session_id: 1 });
    expect(stream.statusTone).toBe("neutral");
  });
});
