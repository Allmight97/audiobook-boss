import { tauriClient } from '../lib/tauri/client';
import type { RuntimeSettingsCapabilities } from '../types/audio';

export const runtimeSettingsCapabilitiesState = $state({
	capabilities: null as RuntimeSettingsCapabilities | null,
	loadError: null as unknown,
	loading: false,
});

const pendingLoads = new Map<string, Promise<RuntimeSettingsCapabilities | null>>();
let lastSuccessfulResult: RuntimeSettingsCapabilities | null = null;
let latestLoadKey: string | null = null;

export function setRuntimeSettingsCapabilities(
	capabilities: RuntimeSettingsCapabilities | null,
): void {
	runtimeSettingsCapabilitiesState.capabilities = capabilities;
	runtimeSettingsCapabilitiesState.loadError = null;
	lastSuccessfulResult = capabilities;
}

export async function hydrateRuntimeSettingsCapabilities(): Promise<RuntimeSettingsCapabilities | null> {
	const key = 'auto-detect';
	const existing = pendingLoads.get(key);
	if (existing) {
		return existing;
	}

	// Serve cached successful result for sequential calls after the first
	// successful hydration. `setRuntimeSettingsCapabilities(null)` clears it.
	if (lastSuccessfulResult) {
		runtimeSettingsCapabilitiesState.capabilities = lastSuccessfulResult;
		return lastSuccessfulResult;
	}

	latestLoadKey = key;
	runtimeSettingsCapabilitiesState.loading = true;
	const promise = tauriClient
		.getRuntimeSettingsCapabilities()
		.then((capabilities) => {
			if (latestLoadKey === key) {
				setRuntimeSettingsCapabilities(capabilities);
			}
			return capabilities;
		})
		.catch((error: unknown) => {
			if (latestLoadKey === key) {
				runtimeSettingsCapabilitiesState.capabilities = null;
				runtimeSettingsCapabilitiesState.loadError = error;
			}
			console.warn('Failed to load runtime settings capabilities:', error);
			return null;
		})
		.finally(() => {
			pendingLoads.delete(key);
			if (latestLoadKey === key) {
				runtimeSettingsCapabilitiesState.loading = false;
			}
		});

	pendingLoads.set(key, promise);
	return promise;
}
