/**
 * Bridge module for Tauri IPC communication.
 *
 * Provides typed access to Tauri's invoke, event listening, window management,
 * autostart plugin, and persistent settings store.
 */

import { invoke } from "@tauri-apps/api/core";
import { listen, type Event, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";
import { Store } from "@tauri-apps/plugin-store";

let storeInstance: Store | null = null;
/** Failed opens are propagated and retried on the next attempt. */
export async function getStore(): Promise<Store> {
  if (!storeInstance) storeInstance = await Store.load("store.bin");
  return storeInstance;
}

export { invoke, listen, getCurrentWindow, enable, disable, isEnabled };
export type { Event, UnlistenFn };
