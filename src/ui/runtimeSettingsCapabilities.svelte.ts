import { tauriClient } from '../lib/tauri/client';
import type { ExternalToolchainPreference, RuntimeSettingsCapabilities } from '../types/audio';

export const runtimeSettingsCapabilitiesState = $state({
	capabilities: null as RuntimeSettingsCapabilities | null,
	loadError: null as unknown,
	loading: false,
});

const pendingLoads = new Map<string, Promise<RuntimeSettingsCapabilities | null>>();
let latestLoadKey: string | null = null;

function capabilityKey(externalToolchain: ExternalToolchainPreference | null): string {
	return JSON.stringify(externalToolchain);
}

export function setRuntimeSettingsCapabilities(
	capabilities: RuntimeSettingsCapabilities | null,
): void {
	runtimeSettingsCapabilitiesState.capabilities = capabilities;
	runtimeSettingsCapabilitiesState.loadError = null;
}

export async function hydrateRuntimeSettingsCapabilities(
	externalToolchain: ExternalToolchainPreference | null,
): Promise<RuntimeSettingsCapabilities | null> {
	const key = capabilityKey(externalToolchain);
	const existing = pendingLoads.get(key);
	if (existing) {
		return existing;
	}

	latestLoadKey = key;
	runtimeSettingsCapabilitiesState.loading = true;
	const promise = tauriClient
		.getRuntimeSettingsCapabilities(externalToolchain)
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
