import type { EncoderDefaults } from '../../types/appSettings';

export async function applyEncodingDefaults(defaults: EncoderDefaults): Promise<void> {
	const [{ hydrateRuntimeSettingsCapabilities }, logic] = await Promise.all([
		import('../runtimeSettingsCapabilities.svelte'),
		import('./logic'),
	]);
	const capabilities = await hydrateRuntimeSettingsCapabilities();
	logic.setRuntimeEncoderSettingsCapabilities(capabilities?.encoder ?? null);
	logic.applyEncodingDefaults(defaults);
}

export { readEncoderDefaultsFromState, readEncodingRequestConfig } from './state.svelte';
