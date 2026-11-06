/** Native capabilities are available only inside the Tauri desktop application. */
import { invoke as nativeInvoke, isTauri } from "@tauri-apps/api/core";
import { listen as nativeListen, type Event, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow as nativeWindow } from "@tauri-apps/api/window";
import {
  enable as nativeEnable,
  disable as nativeDisable,
  isEnabled as nativeIsEnabled,
} from "@tauri-apps/plugin-autostart";
import { Store } from "@tauri-apps/plugin-store";
import { i18n } from "../i18n";

export { isTauri as isNativeRuntime };
export type { Event, UnlistenFn };

function requireNativeRuntime() {
  if (!isTauri()) throw new Error(i18n.global.t("runtime.desktopRequired"));
}

export async function invoke<T>(...args: Parameters<typeof nativeInvoke>): Promise<T> {
  requireNativeRuntime();
  return nativeInvoke<T>(...args);
}

export async function listen<T>(...args: Parameters<typeof nativeListen<T>>): Promise<UnlistenFn> {
  requireNativeRuntime();
  return nativeListen<T>(...args);
}

export function getCurrentWindow() {
  requireNativeRuntime();
  return nativeWindow();
}

export async function enable() {
  requireNativeRuntime();
  return nativeEnable();
}

export async function disable() {
  requireNativeRuntime();
  return nativeDisable();
}

export async function isEnabled() {
  requireNativeRuntime();
  return nativeIsEnabled();
}

let storeInstance: Promise<Store> | null = null;
/** Failed opens are propagated and retried on the next attempt. */
export async function getStore(): Promise<Store> {
  requireNativeRuntime();
  storeInstance ??= Store.load("store.bin", { defaults: {}, autoSave: false }).catch((error) => {
    storeInstance = null;
    throw error;
  });
  return storeInstance;
}
