import { describe, expect, it } from "vitest";
import { sanitizeSupport } from "../sanitize";
describe("support sanitization", () => {
  it("retains dated timestamps while redacting IPv6", () => {
    expect(sanitizeSupport("2026-10-10 08:12:30 peer fe80::a and ::1")).toBe(
      "2026-10-10 08:12:30 peer [address] and [address]",
    );
  });
  it("removes remembered secrets, endpoints and literal IPs while retaining useful levels", () => {
    const text = sanitizeSupport(
      "[error] source.example:1704 rejected key secret-value; packet from 192.168.1.20 and [2001:db8::1]; psk=other",
      ["secret-value", "source.example:1704"],
    );
    expect(text).toContain("[error]");
    expect(text).not.toContain("secret-value");
    expect(text).not.toContain("source.example");
    expect(text).not.toContain("192.168.1.20");
    expect(text).not.toContain("2001:db8");
    expect(text).not.toContain("other");
  });
});
