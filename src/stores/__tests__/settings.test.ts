import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";

const mockStore = new Map<string, unknown>();

// This project's `getStore()` (src/composables/useTauri.ts) calls the named
// `Store.load(...)` static method, not a bare `load` export — mock that shape.
vi.mock("@tauri-apps/plugin-store", () => ({
  Store: {
    load: async () => ({
      get: async (k: string) => mockStore.get(k) ?? null,
      set: async (k: string, v: unknown) => {
        mockStore.set(k, v);
      },
      save: async () => {},
    }),
  },
}));

import { useSettingsStore, clampFixedLatency } from "../settings";

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

describe("fixedLatencyMs", () => {
  beforeEach(() => {
    mockStore.clear();
    setActivePinia(createPinia());
  });

  it("defaults to 250", () => {
    expect(useSettingsStore().fixedLatencyMs).toBe(250);
  });

  it("survives a save/load round-trip", async () => {
    const a = useSettingsStore();
    a.fixedLatencyMs = 400;
    await a.saveSettings();

    setActivePinia(createPinia());
    const b = useSettingsStore();
    await b.loadSettings();
    expect(b.fixedLatencyMs).toBe(400);
  });

  it("clamps an empty-field 0 to the minimum instead of persisting 0", async () => {
    const a = useSettingsStore();
    a.fixedLatencyMs = 0;
    await a.saveSettings();

    setActivePinia(createPinia());
    const b = useSettingsStore();
    await b.loadSettings();
    expect(b.fixedLatencyMs).toBe(50);
  });
});

describe("clampFixedLatency", () => {
  it("clamps an empty-field 0 up to the minimum", () => {
    expect(clampFixedLatency(0)).toBe(50);
  });

  it("falls back to 250 for NaN (a stray '-' or 'e')", () => {
    expect(clampFixedLatency(NaN)).toBe(250);
  });

  it("clamps a value above the maximum down to 2000", () => {
    expect(clampFixedLatency(5000)).toBe(2000);
  });

  it("passes a valid value through unchanged", () => {
    expect(clampFixedLatency(400)).toBe(400);
  });
});

describe("profile isolation", () => {
  beforeEach(() => {
    mockStore.clear();
    setActivePinia(createPinia());
  });
  it("restores empty secrets and devices without inheriting previous values", async () => {
    mockStore.set("profiles", {
      Default: { device: "A", ip: "host", psk: "secret" },
      Empty: { device: "B", ip: "", psk: "" },
    });
    const store = useSettingsStore();
    await store.loadSettings();
    await store.switchProfile("Empty");
    expect(store.deviceName).toBe("B");
    expect(store.ip).toBe("");
    expect(store.psk).toBe("");
  });
  it("clones current unsaved settings when creating a profile", async () => {
    const store = useSettingsStore();
    await store.loadSettings();
    store.ip = "unsaved";
    await store.createProfile("New");
    expect(store.profiles.New.ip).toBe("unsaved");
  });
});

describe("secret persistence", () => {
  beforeEach(() => {
    mockStore.clear();
    setActivePinia(createPinia());
  });
  it("keeps secrets in memory by default and exports no secret or endpoints", async () => {
    const store = useSettingsStore();
    store.psk = "supersecret";
    store.ip = "private.host";
    store.sourceAddr = "private.source:1704";
    store.allowlist = "192.168.1.0/24";
    await store.saveSettings();
    expect((mockStore.get("profiles") as Record<string, { psk: string }>).Default.psk).toBe("");
    expect(store.psk).toBe("supersecret");
    expect(store.exportProfile()).not.toContain("supersecret");
    expect(store.exportProfile()).not.toContain("private.");
    expect(store.exportProfile()).not.toContain("192.168.1");
  });
  it("requires explicit opt-in to persist a secret and clears it when disabled", async () => {
    const store = useSettingsStore();
    store.psk = "remembered";
    store.rememberPsk = true;
    await store.saveSettings();
    expect((mockStore.get("profiles") as Record<string, { psk: string }>).Default.psk).toBe(
      "remembered",
    );
    store.rememberPsk = false;
    await store.saveSettings();
    expect((mockStore.get("profiles") as Record<string, { psk: string }>).Default.psk).toBe("");
  });
});
