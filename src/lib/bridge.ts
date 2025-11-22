/// <reference types="vite/client" />
import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import { listen as tauriListen, UnlistenFn } from '@tauri-apps/api/event';
import { open as tauriOpen, OpenDialogOptions } from '@tauri-apps/plugin-dialog';
import { openPath as tauriOpenExternal } from '@tauri-apps/plugin-opener';

// Check if we are running in a Tauri environment
const isTauri = !!(window as any).__TAURI_INTERNALS__;
console.log(`[Bridge] Initialized. isTauri=${isTauri}, DEV=${import.meta.env.DEV}`);

// Helper to lazily load mocks only in DEV mode
async function getMocks() {
    console.log('[Bridge] Loading mocks...');
    if (import.meta.env.DEV) {
        return await import('./mocks');
    }
    throw new Error('Mocks are not available in production builds');
}

export const bridge = {
    /**
     * Wrapper for Tauri's invoke function
     */
    invoke: async <T>(cmd: string, args?: any): Promise<T> => {
        if (isTauri) {
            return tauriInvoke<T>(cmd, args);
        }

        if (import.meta.env.DEV) {
            const mocks = await getMocks();
            return mocks.mockInvoke<T>(cmd, args);
        }

        console.warn(`[Bridge] Tauri not detected and not in DEV mode. Command '${cmd}' ignored.`);
        return Promise.reject('Tauri API not available');
    },

    /**
     * Wrapper for Tauri's listen function
     */
    listen: async <T>(event: string, handler: (event: { payload: T }) => void): Promise<UnlistenFn> => {
        if (isTauri) {
            return tauriListen<T>(event, handler);
        }

        if (import.meta.env.DEV) {
            const mocks = await getMocks();
            // Adapt the mock unlisten (which returns void) to match UnlistenFn (which returns void)
            const unlisten = await mocks.mockListen(event, handler);
            return unlisten;
        }

        console.warn(`[Bridge] Tauri not detected. Listener for '${event}' ignored.`);
        return () => { };
    },

    /**
     * Wrapper for Tauri's dialog open function
     */
    open: async (options?: OpenDialogOptions): Promise<null | string | string[]> => {
        if (isTauri) {
            return tauriOpen(options);
        }

        if (import.meta.env.DEV) {
            const mocks = await getMocks();
            return mocks.mockOpen(options);
        }

        console.warn('[Bridge] Tauri not detected. Dialog open ignored.');
        return null;
    },

    /**
     * Wrapper for Tauri's open (external) function
     */
    openExternal: async (path: string): Promise<void> => {
        if (isTauri) {
            return tauriOpenExternal(path);
        }

        if (import.meta.env.DEV) {
            const mocks = await getMocks();
            return mocks.mockOpenExternal(path);
        }

        console.warn(`[Bridge] Tauri not detected. External open for '${path}' ignored.`);
    }
};
