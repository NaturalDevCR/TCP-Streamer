import { h, nextTick, ref } from "vue";
import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import { i18n } from "../../../i18n";
import Field from "../Field.vue";
import Input from "../Input.vue";
import Select from "../Select.vue";
import SelectOption from "../SelectOption.vue";
import SegmentedControl from "../SegmentedControl.vue";

describe("real accessible controls", () => {
  it("associates generated labels and descriptions with controls", () => {
    const wrapper = mount(Field, {
      props: { label: "Endpoint", help: "Host and port", error: "Required" },
      slots: { default: () => h(Input) },
      global: { plugins: [i18n] },
    });
    const input = wrapper.get("input");
    expect(wrapper.get("label").attributes("for")).toBe(input.attributes("id"));
    expect(input.attributes("aria-describedby")).toContain(
      wrapper.get("[role=alert]").attributes("id"),
    );
    expect(input.attributes("aria-invalid")).toBe("true");
  });
  it("opens a real empty select without rendering forbidden empty items", async () => {
    const wrapper = mount(Select, {
      attachTo: document.body,
      props: { placeholder: "No devices" },
      slots: { default: () => h(SelectOption, { value: "", label: "No devices" }) },
    });
    await wrapper.get("button").trigger("keydown", { key: "ArrowDown" });
    await nextTick();
    expect(wrapper.get("button").attributes("aria-expanded")).toBe("true");
    expect(document.querySelectorAll("[role=option]")).toHaveLength(0);
    wrapper.unmount();
  });
  it("refreshes the selected label when language changes", async () => {
    const label = ref("Client");
    const wrapper = mount(Select, {
      props: { modelValue: "client" },
      slots: { default: () => h(SelectOption, { value: "client", label: label.value }) },
    });
    await nextTick();
    expect(wrapper.get("button").text()).toContain("Client");
    label.value = "Cliente";
    await nextTick();
    await nextTick();
    expect(wrapper.get("button").text()).toContain("Cliente");
    wrapper.unmount();
  });
  it("supports keyboard selection with a single radio tab stop", async () => {
    const wrapper = mount(SegmentedControl, {
      props: {
        modelValue: "source",
        options: [
          { value: "source", label: "Source" },
          { value: "sink", label: "Receiver" },
        ],
      },
    });
    expect(wrapper.findAll('[role=radio][tabindex="0"]')).toHaveLength(1);
    await wrapper.get("[role=radio]").trigger("keydown", { key: "ArrowRight" });
    expect(wrapper.emitted("update:modelValue")?.[0]).toEqual(["sink"]);
  });
});
