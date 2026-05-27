import { tauriClient } from '../../lib/tauri/client';
import type {
	AppSettingsPatch,
	ConcurrencyPreference,
	EncoderDefaults,
	OutputDefaults,
} from '../../types/appSettings';

export async function persistAppSettingsPatch(patch: AppSettingsPatch): Promise<void> {
	try {
		await tauriClient.updateAppSettings(patch);
	} catch (error) {
		console.warn('Failed to persist app settings:', error);
	}
}

export function persistConcurrencyPreference(preference: ConcurrencyPreference): Promise<void> {
	return persistAppSettingsPatch({ maxConcurrentJobs: preference });
}

export function persistEncoderDefaults(defaults: EncoderDefaults): Promise<void> {
	return persistAppSettingsPatch({ encoderDefaults: defaults });
}

export function persistOutputDefaults(defaults: OutputDefaults): Promise<void> {
	return persistAppSettingsPatch({ outputDefaults: defaults });
}
