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

interface StoreFallback {
  get: () => Promise<null>;
  set: () => Promise<void>;
  save: () => Promise<void>;
  delete: () => Promise<void>;
}

type StoreType = Store | StoreFallback;

let storeInstance: StoreType | null = null;

/**
 * Lazily initializes and returns the persistent settings store.
 * Falls back to a no-op implementation if the Store cannot be loaded.
 */
export async function getStore(): Promise<StoreType> {
  if (!storeInstance) {
    try {
      storeInstance = await Store.load("store.bin");
    } catch {
      storeInstance = {
        get: async () => null,
        set: async () => {},
        save: async () => {},
        delete: async () => {},
      };
    }
  }
  return storeInstance;
}

export { invoke, listen, getCurrentWindow, enable, disable, isEnabled };
export type { Event, UnlistenFn };
