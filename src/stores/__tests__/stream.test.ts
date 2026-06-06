import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { useStreamStore } from "../stream";

describe("useStreamStore", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    setActivePinia(createPinia());
  });

  it("initializes with default values", () => {
    const store = useStreamStore();
    expect(store.isStreaming).toBe(false);
    expect(store.statusText).toBe("Ready");
    expect(store.uptimeSeconds).toBe(0);
    expect(store.bytesSent).toBe(0);
    expect(store.bitrateKbps).toBe(0);
    expect(store.qualityScore).toBe(0);
    expect(store.jitter).toBe(0);
    expect(store.logs).toEqual([]);
    expect(store.toasts).toEqual([]);
  });

  it("addToast adds a toast with correct type", () => {
    const store = useStreamStore();
    store.addToast("Test message", "success");
    expect(store.toasts).toHaveLength(1);
    expect(store.toasts[0].message).toBe("Test message");
    expect(store.toasts[0].type).toBe("success");
    expect(store.toasts[0].id).toBe(1);
  });

  it("addToast removes toast after timeout", () => {
    const store = useStreamStore();
    store.addToast("Message", "info");
    expect(store.toasts).toHaveLength(1);
    vi.advanceTimersByTime(3500);
    expect(store.toasts).toHaveLength(0);
  });

  it("addToast increments toast id", () => {
    const store = useStreamStore();
    store.addToast("First");
    store.addToast("Second");
    expect(store.toasts).toHaveLength(2);
    expect(store.toasts[0].id).toBe(1);
    expect(store.toasts[1].id).toBe(2);
  });

  it("clearLogs empties log array", () => {
    const store = useStreamStore();
    store.logs.push({
      timestamp: "12:00:00",
      level: "info",
      message: "Test log",
      visible: true,
    });
    expect(store.logs).toHaveLength(1);
    store.clearLogs();
    expect(store.logs).toHaveLength(0);
  });

  it("filteredLogs respects logFilter", () => {
    const store = useStreamStore();
    store.logs.push(
      { timestamp: "12:00:00", level: "info", message: "Info log", visible: true },
      { timestamp: "12:00:01", level: "error", message: "Error log", visible: true },
    );
    expect(store.filteredLogs).toHaveLength(2);
    store.logFilter = "error";
    expect(store.filteredLogs).toHaveLength(1);
    expect(store.filteredLogs[0].level).toBe("error");
  });

  it("formattedUptime formats seconds correctly", () => {
    const store = useStreamStore();
    store.uptimeSeconds = 65;
    expect(store.formattedUptime).toBe("1:05");
    store.uptimeSeconds = 3661;
    expect(store.formattedUptime).toBe("1:01:01");
  });

  it("formattedBytes formats bytes correctly", () => {
    const store = useStreamStore();
    store.bytesSent = 0;
    expect(store.formattedBytes).toBe("0 MB");
    store.bytesSent = 512 * 1024;
    expect(store.formattedBytes).toBe("512.0 KB");
    store.bytesSent = 5 * 1024 * 1024;
    expect(store.formattedBytes).toBe("5.00 MB");
  });

  it("qualityLabel returns correct labels", () => {
    const store = useStreamStore();
    store.qualityScore = 95;
    expect(store.qualityLabel).toEqual({ text: "Excellent", color: "#10b981" });
    store.qualityScore = 75;
    expect(store.qualityLabel).toEqual({ text: "Good", color: "#f59e0b" });
    store.qualityScore = 55;
    expect(store.qualityLabel).toEqual({ text: "Fair", color: "#f97316" });
    store.qualityScore = 30;
    expect(store.qualityLabel).toEqual({ text: "Poor", color: "#ef4444" });
  });
});
