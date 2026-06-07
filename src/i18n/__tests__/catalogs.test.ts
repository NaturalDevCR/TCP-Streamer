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
});
