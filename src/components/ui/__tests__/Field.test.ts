import { mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it } from "vitest";
import { i18n } from "../../../i18n";
import Field from "../Field.vue";

const TooltipStub = {
  template: `
    <div data-test="tooltip">
      <slot name="trigger" />
      <div data-test="tooltip-content"><slot /></div>
    </div>
  `,
};

describe("Field", () => {
  beforeEach(() => {
    i18n.global.locale.value = "en";
  });

  it("renders accessible tooltip help next to the field label", () => {
    const wrapper = mount(Field, {
      props: {
        id: "sample-rate",
        label: "Output Sample Rate",
        tooltip: "Controls the wire rate.",
      },
      slots: {
        default: "<input id='sample-rate' />",
      },
      global: {
        plugins: [i18n],
        stubs: { Tooltip: TooltipStub },
      },
    });

    expect(wrapper.get("label").attributes("for")).toBe("sample-rate");
    expect(wrapper.get("button").attributes("type")).toBe("button");
    expect(wrapper.get("button").attributes("aria-label")).toBe(
      "More information about Output Sample Rate",
    );
    expect(wrapper.get('[data-test="tooltip-content"]').text()).toBe("Controls the wire rate.");
  });

  it("keeps field text rendered as a label when no id is provided", () => {
    const wrapper = mount(Field, {
      props: { label: "Transport" },
      slots: { default: "<select />" },
      global: {
        plugins: [i18n],
        stubs: { Tooltip: TooltipStub },
      },
    });

    expect(wrapper.find("label").exists()).toBe(true);
  });
});
