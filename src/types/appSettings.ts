import type {
	AppSettings as GeneratedAppSettings,
	AppSettingsPatch as GeneratedAppSettingsPatch,
	ConcurrencyPreference as GeneratedConcurrencyPreference,
	DensityPreference as GeneratedDensityPreference,
	EncoderDefaults as GeneratedEncoderDefaults,
	OutputDefaults as GeneratedOutputDefaults,
	StartupBehavior as GeneratedStartupBehavior,
	ToolchainPreferences as GeneratedToolchainPreferences,
} from '../lib/generated/tauri';
import type { EncoderSettings, OutputNamingConfig } from './audio';
import type { NullToOptionalDeep } from './ipc';

export type ConcurrencyPreference = GeneratedConcurrencyPreference;
export type DensityPreference = GeneratedDensityPreference;
export type StartupBehavior = GeneratedStartupBehavior;
export type ToolchainPreferences = NullToOptionalDeep<GeneratedToolchainPreferences>;
export type EncoderDefaults = Omit<NullToOptionalDeep<GeneratedEncoderDefaults>, 'settings'> & {
	settings: EncoderSettings;
};
export type OutputDefaults = NullToOptionalDeep<GeneratedOutputDefaults>;
export type PinnedDefaults = {
	maxConcurrentJobs: ConcurrencyPreference;
	encoderDefaults: EncoderDefaults;
	outputDefaults: OutputDefaults;
};
export type AppSettings = Omit<
	NullToOptionalDeep<GeneratedAppSettings>,
	'encoderDefaults' | 'outputDefaults' | 'pinnedDefaults'
> & {
	encoderDefaults: EncoderDefaults;
	outputDefaults: OutputDefaults;
	pinnedDefaults?: PinnedDefaults;
};

export type AppSettingsPatch = Partial<{
	maxConcurrentJobs: GeneratedAppSettingsPatch['maxConcurrentJobs'];
	encoderDefaults: EncoderDefaults | null;
	outputDefaults:
		| (Omit<OutputDefaults, 'outputNaming'> & { outputNaming: OutputNamingConfig })
		| null;
	toolchain: ToolchainPreferences | null;
	startupBehavior: StartupBehavior | null;
	density: DensityPreference | null;
	pinnedDefaults: PinnedDefaults | null;
}>;
