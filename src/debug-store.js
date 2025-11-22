import * as storePlugin from '@tauri-apps/plugin-store';
console.log('Plugin Store Exports:', Object.keys(storePlugin));
if (storePlugin.Store) {
    console.log('Store prototype:', Object.getOwnPropertyNames(storePlugin.Store.prototype));
}
