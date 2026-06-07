import { setActivePinia, createPinia } from "pinia";
import { beforeEach, describe, expect, it } from "vitest";
import { useUiStore } from "../ui";

describe("ui store", () => {
  beforeEach(() => setActivePinia(createPinia()));
  it("defaults section to dashboard and toggles locale", () => {
    const ui = useUiStore();
    expect(ui.section).toBe("dashboard");
    ui.setLocale("es");
    expect(ui.locale).toBe("es");
  });
});
