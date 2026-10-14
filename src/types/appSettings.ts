import type {
	AacDecoder as GeneratedAacDecoder,
	AcquisitionLane as GeneratedAcquisitionLane,
	AppSettings as GeneratedAppSettings,
	ConcurrencyPreference as GeneratedConcurrencyPreference,
	EncoderDefaults as GeneratedEncoderDefaults,
	OutputDefaults as GeneratedOutputDefaults,
	SettingsIntent as GeneratedSettingsIntent,
	SettingsOutcome as GeneratedSettingsOutcome,
	SettingsSnapshot as GeneratedSettingsSnapshot,
	StartupBehavior as GeneratedStartupBehavior,
} from '../lib/generated/tauri';
import type { EncoderSettings } from './audio';
import type { NullToOptionalDeep } from './ipc';

export type ConcurrencyPreference = GeneratedConcurrencyPreference;
export type AcquisitionLane = GeneratedAcquisitionLane;
export type AacDecoder = GeneratedAacDecoder;
export type StartupBehavior = GeneratedStartupBehavior;
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

/** The settings in effect and whether they are saved. Rust owns the shape. */
export type SettingsSnapshot = Omit<
	NullToOptionalDeep<GeneratedSettingsSnapshot>,
	'settings' | 'startupDefaults'
> & {
	settings?: AppSettings;
	startupDefaults?: PinnedDefaults;
};

export type SettingsIntent =
	| Exclude<GeneratedSettingsIntent, { kind: 'remember' }>
	| {
			kind: 'remember';
			encoderDefaults?: EncoderDefaults;
			outputDefaults?: OutputDefaults;
			defaultAcquisitionLane?: AcquisitionLane;
	  };
export type SettingsOutcome = GeneratedSettingsOutcome;
export type SettingsReply = { outcome: SettingsOutcome; snapshot: SettingsSnapshot };
