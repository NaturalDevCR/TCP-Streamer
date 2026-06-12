import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { beforeEach, describe, expect, it } from "vitest";
import { i18n } from "../../../i18n";
import SettingLabel from "../SettingLabel.vue";

const TooltipStub = {
  template: `
    <div data-test="tooltip">
      <slot name="trigger" />
      <div data-test="tooltip-content"><slot /></div>
    </div>
  `,
};

describe("SettingLabel", () => {
  beforeEach(() => {
    i18n.global.locale.value = "en";
  });

  it("uses non-label text for switch rows without a control id", () => {
    const wrapper = mount(SettingLabel, {
      props: {
        label: "Auto-stream on load",
        tooltip: "Starts streaming after the application loads.",
      },
      global: {
        plugins: [i18n],
        stubs: { Tooltip: TooltipStub },
      },
    });

    expect(wrapper.find("label").exists()).toBe(false);
    expect(wrapper.get("span").text()).toContain("Auto-stream on load");
    expect(wrapper.get("button").attributes("aria-label")).toBe(
      "More information about Auto-stream on load",
    );
  });

  it("does not render a tooltip trigger when no help text is provided", () => {
    const wrapper = mount(SettingLabel, {
      props: { label: "Language" },
      global: {
        plugins: [i18n],
        stubs: { Tooltip: TooltipStub },
      },
    });

    expect(wrapper.find("button").exists()).toBe(false);
  });

  it("opens the real tooltip when its button receives keyboard focus", async () => {
    const wrapper = mount(SettingLabel, {
      attachTo: document.body,
      props: {
        label: "Transport",
        tooltip: "TCP maximizes compatibility.",
      },
      global: { plugins: [i18n] },
    });

    await wrapper.get("button").trigger("focus");
    await nextTick();

    expect(document.body.textContent).toContain("TCP maximizes compatibility.");
    wrapper.unmount();
  });
});
