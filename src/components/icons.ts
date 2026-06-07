import { h, type Component } from "vue";

const svgAttrs = {
  xmlns: "http://www.w3.org/2000/svg",
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  "stroke-width": 2,
  "stroke-linecap": "round",
  "stroke-linejoin": "round",
  width: 18,
  height: 18,
} as const;

export const IconDashboard: Component = {
  render: () =>
    h("svg", svgAttrs, [
      h("rect", { width: 7, height: 9, x: 3, y: 3, rx: 1 }),
      h("rect", { width: 7, height: 5, x: 14, y: 3, rx: 1 }),
      h("rect", { width: 7, height: 9, x: 14, y: 12, rx: 1 }),
      h("rect", { width: 7, height: 5, x: 3, y: 16, rx: 1 }),
    ]),
};

export const IconConnection: Component = {
  render: () =>
    h("svg", svgAttrs, [
      h("path", { d: "M12 20h.01" }),
      h("path", { d: "M2 20h.01" }),
      h("path", { d: "M22 20h.01" }),
      h("path", { d: "M5 12a7 7 0 0 1 14 0" }),
      h("path", { d: "M8.5 15.5a3.5 3.5 0 0 1 7 0" }),
    ]),
};

export const IconAudio: Component = {
  render: () =>
    h("svg", svgAttrs, [
      h("path", { d: "M11 5L6 9H2v6h4l5 4V5z" }),
      h("path", { d: "M19.07 4.93a10 10 0 0 1 0 14.14M15.54 8.46a5 5 0 0 1 0 7.07" }),
    ]),
};

export const IconLogs: Component = {
  render: () =>
    h("svg", svgAttrs, [
      h("path", { d: "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" }),
      h("polyline", { points: "14 2 14 8 20 8" }),
      h("line", { x1: 16, y1: 13, x2: 8, y2: 13 }),
      h("line", { x1: 16, y1: 17, x2: 8, y2: 17 }),
    ]),
};

export const IconSettings: Component = {
  render: () =>
    h("svg", svgAttrs, [
      h("path", {
        d: "M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.1a2 2 0 0 1-1-1.72v-.51a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z",
      }),
      h("circle", { cx: 12, cy: 12, r: 3 }),
    ]),
};

export const IconPlay: Component = {
  render: () => h("svg", svgAttrs, [h("polygon", { points: "5 3 19 12 5 21 5 3" })]),
};

export const IconStop: Component = {
  render: () => h("svg", svgAttrs, [h("rect", { x: 3, y: 3, width: 18, height: 18, rx: 2 })]),
};

export const IconCopy: Component = {
  render: () =>
    h("svg", svgAttrs, [
      h("rect", { x: 9, y: 9, width: 13, height: 13, rx: 2 }),
      h("path", { d: "M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" }),
    ]),
};

export const IconSearch: Component = {
  render: () =>
    h("svg", svgAttrs, [
      h("circle", { cx: 11, cy: 11, r: 8 }),
      h("path", { d: "m21 21-4.35-4.35" }),
    ]),
};

export const IconTrash: Component = {
  render: () =>
    h("svg", svgAttrs, [
      h("path", { d: "M3 6h18" }),
      h("path", { d: "M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6" }),
      h("path", { d: "M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2" }),
    ]),
};

export const IconPlus: Component = {
  render: () =>
    h("svg", svgAttrs, [
      h("line", { x1: 12, y1: 5, x2: 12, y2: 19 }),
      h("line", { x1: 5, y1: 12, x2: 19, y2: 12 }),
    ]),
};

export const IconChevronDown: Component = {
  render: () => h("svg", svgAttrs, [h("path", { d: "m6 9 6 6 6-6" })]),
};

export const IconCheck: Component = {
  render: () => h("svg", svgAttrs, [h("path", { d: "M20 6 9 17l-5-5" })]),
};

export const IconGlobe: Component = {
  render: () =>
    h("svg", svgAttrs, [
      h("circle", { cx: 12, cy: 12, r: 10 }),
      h("path", { d: "M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20" }),
      h("path", { d: "M2 12h20" }),
    ]),
};

export const IconLock: Component = {
  render: () =>
    h("svg", svgAttrs, [
      h("rect", { x: 3, y: 11, width: 18, height: 11, rx: 2 }),
      h("path", { d: "M7 11V7a5 5 0 0 1 10 0v4" }),
    ]),
};
