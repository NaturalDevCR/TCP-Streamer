import { describe, it, expect, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { useSettingsStore } from "../settings";

describe("useSettingsStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it("initializes with default values", () => {
    const store = useSettingsStore();
    expect(store.devices).toEqual([]);
    expect(store.osType).toBe("unknown");
    expect(store.localIp).toBe("127.0.0.1");
    expect(store.mode).toBe("client");
    expect(store.port).toBe(1704);
    expect(store.sampleRate).toBe(48000);
    expect(store.bufferSize).toBe(1024);
    expect(store.ringBufferDuration).toBe(4000);
    expect(store.format).toBe("pcm");
    expect(store.highPriority).toBe(true);
    expect(store.dscpStrategy).toBe("voip");
    expect(store.chunkSize).toBe(512);
    expect(store.currentProfile).toBe("Default");
  });

  it("computes isServer correctly", () => {
    const store = useSettingsStore();
    expect(store.isServer).toBe(false);
    store.mode = "server";
    expect(store.isServer).toBe(true);
  });

  it("computes isLoopback correctly", () => {
    const store = useSettingsStore();
    expect(store.isLoopback).toBe(false);
    store.deviceName = "[Loopback] Speakers";
    expect(store.isLoopback).toBe(true);
  });

  it("computes showLoopback correctly", () => {
    const store = useSettingsStore();
    expect(store.showLoopback).toBe(false);
    store.osType = "windows";
    expect(store.showLoopback).toBe(true);
  });

  it("initializes with default latency profile", () => {
    const store = useSettingsStore();
    expect(store.latencyProfile).toBe("balanced");
  });

  it("supports changing latency profile", () => {
    const store = useSettingsStore();
    store.latencyProfile = "ultra-low";
    expect(store.latencyProfile).toBe("ultra-low");
    store.latencyProfile = "robust";
    expect(store.latencyProfile).toBe("robust");
  });

  it("allowlist defaults to empty", () => {
    const store = useSettingsStore();
    expect(store.allowlist).toBe("");
  });

  it("allowlist can be set to CIDR rules", () => {
    const store = useSettingsStore();
    store.allowlist = "192.168.1.0/24, 10.0.0.5";
    expect(store.allowlist).toBe("192.168.1.0/24, 10.0.0.5");
  });

  it("uses a DSCP strategy key the backend recognizes", () => {
    const store = useSettingsStore();
    const validKeys = ["voip", "ef", "cs5", "lowdelay", "throughput", "besteffort"];
    expect(validKeys).toContain(store.dscpStrategy);
  });
});
