import { describe, expect, it } from "vitest";
import audioSection from "../AudioSection.vue?raw";
import connectionSection from "../ConnectionSection.vue?raw";
import settingsSection from "../SettingsSection.vue?raw";

const sections = [
  [
    "ConnectionSection",
    connectionSection,
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
    "AudioSection",
    audioSection,
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
  [
    "SettingsSection",
    settingsSection,
    ["profile", "autoStart", "autoStream", "autoReconnect", "language"],
  ],
] as const;

describe("configurable option help coverage", () => {
  it.each(sections)("%s uses every required help key", (_section, source, keys) => {
    const usedKeys = [
      ...source.matchAll(/t\(["'](?:connection|audio|settings)\.help\.([^"']+)["']\)/g),
    ]
      .map((match) => match[1])
      .sort();

    expect([...new Set(usedKeys)]).toEqual([...keys].sort());
  });
});
