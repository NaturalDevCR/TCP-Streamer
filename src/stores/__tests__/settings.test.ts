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

  it("applyPreset changes settings for ethernet", () => {
    const store = useSettingsStore();
    store.applyPreset("ethernet");
    expect(store.ringBufferDuration).toBe(2000);
    expect(store.chunkSize).toBe(512);
    expect(store.minBuffer).toBe(2000);
    expect(store.maxBuffer).toBe(6000);
    expect(store.adaptiveBuffer).toBe(true);
    expect(store.networkPreset).toBe("ethernet");
  });

  it("applyPreset changes settings for wifi", () => {
    const store = useSettingsStore();
    store.applyPreset("wifi");
    expect(store.ringBufferDuration).toBe(4000);
    expect(store.chunkSize).toBe(1024);
    expect(store.minBuffer).toBe(3000);
    expect(store.maxBuffer).toBe(10000);
  });

  it("applyPreset does nothing for custom", () => {
    const store = useSettingsStore();
    const original = store.ringBufferDuration;
    store.applyPreset("custom");
    expect(store.ringBufferDuration).toBe(original);
  });

  it("applyPreset does nothing for invalid preset", () => {
    const store = useSettingsStore();
    const original = store.ringBufferDuration;
    store.applyPreset("nonexistent");
    expect(store.ringBufferDuration).toBe(original);
  });

  it("uses a DSCP strategy key the backend recognizes", () => {
    const store = useSettingsStore();
    const validKeys = ["voip", "ef", "cs5", "lowdelay", "throughput", "besteffort"];
    expect(validKeys).toContain(store.dscpStrategy);
  });
});
