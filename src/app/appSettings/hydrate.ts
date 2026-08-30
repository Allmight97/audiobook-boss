import type { PinnedDefaults } from '../../types/appSettings';
import { liveSettingsCapability } from '../../lib/tauri/capabilities/settings';
import { applyEncodingDefaults } from '../../ui/encoderPanel';
import { resolveStartupDefaults } from './startupDefaults';

let hydrationPromise: Promise<PinnedDefaults | null> | null = null;

export function hydrateAppSettingsProduction(): Promise<PinnedDefaults | null> {
	hydrationPromise ??= hydrateOnce().finally(() => {
		hydrationPromise = null;
	});
	return hydrationPromise;
}

async function hydrateOnce(): Promise<PinnedDefaults | null> {
	const defaults = await resolveStartupDefaults(liveSettingsCapability).catch((error: unknown) => {
		console.warn('Failed to fetch app settings:', error);
		return null;
	});
	if (defaults === null) {
		return null;
	}

	const capabilities = await liveSettingsCapability
		.getRuntimeSettingsCapabilities()
		.catch((error: unknown) => {
			console.warn('Failed to load runtime settings capabilities:', error);
			return null;
		});

	try {
		await applyEncodingDefaults(defaults.encoderDefaults, capabilities?.encoder ?? null);
	} catch (error) {
		console.warn('Failed to hydrate encoding defaults:', error);
	}

	return defaults;
}
