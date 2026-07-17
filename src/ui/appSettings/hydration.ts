import { tauriClient } from '../../lib/tauri/client';
import type { AppSettings } from '../../types/appSettings';
import { applyEncodingDefaults } from '../encoderPanel';
import { applyMaxConcurrentPreference } from '../jobControls';
import { applyOutputDefaultsFromSettings } from '../outputPanel';
import { loadRuntimeSettingsCapabilities } from '../runtimeSettingsCapabilities.svelte';
import { applyDensityPreference, applyRailWidthPreference } from '../appShell';
import { applyEditSurfacePreference } from '../metadataSurface';

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

	// Density and edit surface are global user intent, not panel defaults.
	// They always come from the top-level values even when panel hydration
	// uses pinned defaults.
	applyDensityPreference(settings.density ?? 'comfortable');
	applyEditSurfacePreference(settings.editSurface ?? 'rail');
	applyRailWidthPreference(settings.railWidth ?? 420);

	const capabilities = await loadRuntimeSettingsCapabilities();

	// Startup source selection: the panels always persist their last-used
	// values at the top level; the pinned snapshot only wins when the user
	// chose "use my pinned defaults" AND has actually pinned one.
	const source =
		settings.startupBehavior === 'pinnedDefaults' && settings.pinnedDefaults
			? settings.pinnedDefaults
			: settings;

	try {
		await applyEncodingDefaults(source.encoderDefaults, capabilities?.encoder ?? null);
	} catch (error) {
		console.warn('Failed to hydrate encoding defaults:', error);
	}

	try {
		applyOutputDefaultsFromSettings(source.outputDefaults);
	} catch (error) {
		console.warn('Failed to hydrate output defaults:', error);
	}

	try {
		await applyMaxConcurrentPreference(
			source.maxConcurrentJobs,
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
