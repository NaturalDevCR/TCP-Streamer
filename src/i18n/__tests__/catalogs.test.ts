import { describe, it, expect } from "vitest";
import en from "../en.json";
import es from "../es.json";

function keys(o: Record<string, unknown>, p = ""): string[] {
  return Object.entries(o).flatMap(([k, v]) =>
    typeof v === "object" && v !== null
      ? keys(v as Record<string, unknown>, `${p}${k}.`)
      : [`${p}${k}`],
  );
}

describe("i18n catalog parity", () => {
  it("en and es catalogs have the same keys", () => {
    expect(keys(en).sort()).toEqual(keys(es).sort());
  });

  it.each([
    [
      "connection",
      [
        "role",
        "inputDevice",
        "loopback",
        "transport",
        "mode",
        "psk",
        "targetAddress",
        "port",
        "allowlist",
        "outputDevice",
        "sourceAddress",
        "discoveredSources",
      ],
    ],
    [
      "audio",
      [
        "inputDevice",
        "sampleRate",
        "bufferSize",
        "format",
        "loopback",
        "latencyProfile",
        "ringBufferDuration",
        "chunkSize",
        "adaptiveBuffer",
        "minBuffer",
        "maxBuffer",
      ],
    ],
    ["settings", ["profile", "autoStart", "autoStream", "autoReconnect", "language"]],
  ] as const)("%s exposes help for every configurable option", (section, expectedKeys) => {
    for (const catalog of [en, es]) {
      const help = catalog[section].help;
      expect(help).toBeDefined();
      if (!help) continue;
      expect(Object.keys(help).sort()).toEqual([...expectedKeys].sort());
      expect(Object.values(help).every((value) => value.trim().length > 20)).toBe(true);
    }
  });
});
