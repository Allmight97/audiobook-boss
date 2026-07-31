import type { EncoderDefaults } from '../../types/appSettings';
import type { EncoderSettingsCapabilities } from '../../types/audio';

export async function applyEncodingDefaults(
	defaults: EncoderDefaults,
	capabilities: EncoderSettingsCapabilities | null,
): Promise<void> {
	const logic = await import('./logic');
	logic.setRuntimeEncoderSettingsCapabilities(capabilities);
	logic.applyEncodingDefaults(defaults);
}

export { readEncoderDefaultsFromState, readEncodingRequestConfig } from './state.svelte';
