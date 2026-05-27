import type {
	AppSettings as GeneratedAppSettings,
	AppSettingsPatch as GeneratedAppSettingsPatch,
	ConcurrencyPreference as GeneratedConcurrencyPreference,
	EncoderDefaults as GeneratedEncoderDefaults,
	OutputDefaults as GeneratedOutputDefaults,
} from '../lib/generated/tauri';
import type { EncoderSettings, ExternalToolchainPreference, OutputNamingConfig } from './audio';
import type { NullToOptionalDeep } from './ipc';

export type ConcurrencyPreference = GeneratedConcurrencyPreference;
export type EncoderDefaults = Omit<
	NullToOptionalDeep<GeneratedEncoderDefaults>,
	'settings' | 'externalToolchain'
> & {
	settings: EncoderSettings;
	externalToolchain: ExternalToolchainPreference;
};
export type OutputDefaults = NullToOptionalDeep<GeneratedOutputDefaults>;
export type AppSettings = Omit<
	NullToOptionalDeep<GeneratedAppSettings>,
	'encoderDefaults' | 'outputDefaults'
> & {
	encoderDefaults: EncoderDefaults;
	outputDefaults: OutputDefaults;
};

export type AppSettingsPatch = Partial<{
	maxConcurrentJobs: GeneratedAppSettingsPatch['maxConcurrentJobs'];
	encoderDefaults: EncoderDefaults | null;
	outputDefaults:
		| (Omit<OutputDefaults, 'outputNaming'> & { outputNaming: OutputNamingConfig })
		| null;
}>;
