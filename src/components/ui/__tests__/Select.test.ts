import { h } from "vue";
import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import Select from "../Select.vue";
import SelectOption from "../SelectOption.vue";

// Note: this is a render smoke test (mounts cleanly, shows the placeholder in
// the closed state). It does NOT exercise opening the dropdown — Reka UI's
// portaled, layout-measured content is not reliably testable in happy-dom.
// The "does it open" verification is manual.
describe("Select", () => {
  it("mounts with options and shows the placeholder when empty", () => {
    const wrapper = mount(Select, {
      props: { placeholder: "Pick one" },
      slots: {
        default: () => [
          h(SelectOption, { value: "a", label: "Option A" }),
          h(SelectOption, { value: 1, label: "Option 1" }),
        ],
      },
    });
    expect(wrapper.html()).toContain("Pick one");
  });
});
