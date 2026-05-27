import type { EncoderDefaults } from '../../types/appSettings';

export async function applyEncodingDefaults(defaults: EncoderDefaults): Promise<void> {
	const logic = await import('./logic');
	logic.applyEncodingDefaults(defaults);
}

export { readEncoderDefaultsFromState, readEncodingRequestConfig } from './state.svelte';
