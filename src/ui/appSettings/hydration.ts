import { tauriClient } from '../../lib/tauri/client';
import type { AppSettings } from '../../types/appSettings';
import { applyEncodingDefaults } from '../encoderPanel';
import { applyMaxConcurrentPreference } from '../jobControls';
import { applyOutputDefaultsFromSettings } from '../outputPanel';
import { loadRuntimeSettingsCapabilities } from '../runtimeSettingsCapabilities.svelte';

let hydrationPromise: Promise<void> | null = null;

export function hydrateAppSettings(): Promise<void> {
	hydrationPromise ??= hydrateOnce().finally(() => {
		hydrationPromise = null;
	});
	return hydrationPromise;
}

export const initializeAppSettingsControlPlane = hydrateAppSettings;

async function hydrateOnce(): Promise<void> {
	const settings = await loadAppSettings();
	if (!settings) {
		return;
	}

	const capabilities = await loadRuntimeSettingsCapabilities();

	try {
		await applyEncodingDefaults(settings.encoderDefaults, capabilities?.encoder ?? null);
	} catch (error) {
		console.warn('Failed to hydrate encoding defaults:', error);
	}

	try {
		applyOutputDefaultsFromSettings(settings.outputDefaults);
	} catch (error) {
		console.warn('Failed to hydrate output defaults:', error);
	}

	try {
		await applyMaxConcurrentPreference(
			settings.maxConcurrentJobs,
			capabilities?.maxConcurrentJobs ?? null,
		);
	} catch (error) {
		console.warn('Failed to hydrate max concurrency preference:', error);
	}
}

async function loadAppSettings(): Promise<AppSettings | null> {
	try {
		return await tauriClient.getAppSettings();
	} catch (error) {
		console.warn('Failed to fetch app settings:', error);
		return null;
	}
}
