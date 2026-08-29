import type { SettingsCapability } from '../../lib/tauri/capabilities/settings';
import type { PinnedDefaults } from '../../types/appSettings';

/**
 * The panels auto-persist every change into the top-level (last-used) values,
 * so the pinned snapshot wins only when the user chose that startup behavior
 * AND has actually pinned one. Encoder and concurrency hydration must agree on
 * this selection, so it lives here rather than at each call site.
 */
export async function resolveStartupDefaults(
	capability: SettingsCapability,
): Promise<PinnedDefaults> {
	const settings = await capability.getAppSettings();
	return settings.startupBehavior === 'pinnedDefaults' && settings.pinnedDefaults
		? settings.pinnedDefaults
		: settings;
}
