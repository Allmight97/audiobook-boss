import { tauriClient } from '../lib/tauri/client';
import type { RuntimeSettingsCapabilities } from '../types/audio';

export const runtimeSettingsCapabilitiesState = $state({
	capabilities: null as RuntimeSettingsCapabilities | null,
	loadError: null as unknown,
	loading: false,
});

let pendingLoad: Promise<RuntimeSettingsCapabilities | null> | null = null;
let latestLoadId = 0;

export function setRuntimeSettingsCapabilities(
	capabilities: RuntimeSettingsCapabilities | null,
): void {
	runtimeSettingsCapabilitiesState.capabilities = capabilities;
	runtimeSettingsCapabilitiesState.loadError = null;
	runtimeSettingsCapabilitiesState.loading = false;
}

export function invalidateRuntimeSettingsCapabilities(): void {
	latestLoadId += 1;
	pendingLoad = null;
	runtimeSettingsCapabilitiesState.capabilities = null;
	runtimeSettingsCapabilitiesState.loadError = null;
	runtimeSettingsCapabilitiesState.loading = false;
}

export function refreshRuntimeSettingsCapabilities(): Promise<RuntimeSettingsCapabilities | null> {
	return loadRuntimeSettingsCapabilities({ refresh: true });
}

export async function loadRuntimeSettingsCapabilities(
	options: { refresh?: boolean } = {},
): Promise<RuntimeSettingsCapabilities | null> {
	const refresh = options.refresh === true;
	if (pendingLoad) {
		return pendingLoad;
	}

	if (!refresh && runtimeSettingsCapabilitiesState.capabilities) {
		return runtimeSettingsCapabilitiesState.capabilities;
	}

	const loadId = latestLoadId + 1;
	latestLoadId = loadId;
	runtimeSettingsCapabilitiesState.loading = true;
	pendingLoad = tauriClient
		.getRuntimeSettingsCapabilities()
		.then((capabilities) => {
			if (latestLoadId === loadId) {
				setRuntimeSettingsCapabilities(capabilities);
			}
			return capabilities;
		})
		.catch((error: unknown) => {
			if (latestLoadId === loadId) {
				runtimeSettingsCapabilitiesState.capabilities = null;
				runtimeSettingsCapabilitiesState.loadError = error;
			}
			console.warn('Failed to load runtime settings capabilities:', error);
			return null;
		})
		.finally(() => {
			if (latestLoadId === loadId) {
				pendingLoad = null;
				runtimeSettingsCapabilitiesState.loading = false;
			}
		});

	return pendingLoad;
}
