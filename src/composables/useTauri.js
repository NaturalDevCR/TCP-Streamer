import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";
import { Store } from "@tauri-apps/plugin-store";

let storeInstance = null;

export async function getStore() {
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
