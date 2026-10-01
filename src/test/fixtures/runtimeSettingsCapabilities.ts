import type { EncoderAvailability, RuntimeSettingsCapabilities } from '../../types/audio';
import type {
	EncoderAvailability as GeneratedEncoderAvailability,
	EncoderSettingsCapabilities as GeneratedEncoderSettingsCapabilities,
	MaxConcurrentJobsCapabilities as GeneratedMaxConcurrentJobsCapabilities,
	RuntimeSettingsCapabilities as GeneratedRuntimeSettingsCapabilities,
} from '../../lib/generated/tauri';

type RuntimeSettingsCapabilitiesFixtureOverrides = {
	encoder?: Partial<RuntimeSettingsCapabilities['encoder']>;
	maxConcurrentJobs?: Partial<RuntimeSettingsCapabilities['maxConcurrentJobs']>;
};

export function encoderAvailabilityFixture(
	overrides: Partial<EncoderAvailability> = {},
): EncoderAvailability {
	const base = {
		aacAtAvailable: overrides.aacAtAvailable ?? true,
		nativeAacAvailable: overrides.nativeAacAvailable ?? true,
		autoEncoder: 'native_aac' as const,
	} satisfies GeneratedEncoderAvailability;
	return { ...base, ...overrides };
}

export function runtimeSettingsCapabilitiesFixture(
	overrides: RuntimeSettingsCapabilitiesFixtureOverrides = {},
): RuntimeSettingsCapabilities {
	// Build the base object with a satisfies check against the generated type
	// so that generated IPC shape drift (e.g. added/removed/changed fields on
	// RuntimeSettingsCapabilities) fails TypeScript at this literal.
	const base = {
		encoder: {
			availability: {
				aacAtAvailable: true,
				nativeAacAvailable: true,
				autoEncoder: 'native_aac' as const,
			} satisfies GeneratedEncoderAvailability,
			encoderTypes: ['auto', 'aac_at', 'native_aac', 'faac', 'opus'],
			encoderConfigurations: [
				{
					encoderType: 'opus' as const,
					allowedModes: ['vbr' as const],
					defaultMode: { mode: 'vbr_target' as const },
					explicitSampleRates: [8000, 12000, 16000, 24000, 48000],
					faacProfiles: [],
					bitrateKbpsMin: 6,
					bitrateKbpsMax: 510,
				},
				{
					faacProfiles: [],
					bitrateKbpsMin: 1,
					bitrateKbpsMax: 1152,
					encoderType: 'auto' as const,
					allowedModes: ['cbr' as const],
					defaultMode: { mode: 'cbr' as const },
					explicitSampleRates: [
						7350, 8000, 11025, 12000, 16000, 22050, 24000, 32000, 44100, 48000, 64000, 88200, 96000,
					],
				},
				{
					faacProfiles: [],
					bitrateKbpsMin: 1,
					bitrateKbpsMax: 1152,
					encoderType: 'aac_at' as const,
					allowedModes: ['cvbr' as const],
					defaultMode: { mode: 'cvbr' as const },
					explicitSampleRates: [
						7350, 8000, 11025, 12000, 16000, 22050, 24000, 32000, 44100, 48000, 64000, 88200, 96000,
					],
				},
				{
					faacProfiles: [],
					bitrateKbpsMin: 1,
					bitrateKbpsMax: 1152,
					encoderType: 'native_aac' as const,
					allowedModes: ['cbr' as const],
					defaultMode: { mode: 'cbr' as const },
					explicitSampleRates: [
						7350, 8000, 11025, 12000, 16000, 22050, 24000, 32000, 44100, 48000, 64000, 88200, 96000,
					],
				},
				{
					faacProfiles: ['auto', 'aac_lc', 'he_aac_v1'].map((profile) => ({
						profile: profile as 'auto' | 'aac_lc' | 'he_aac_v1',
						explicitSampleRates:
							profile === 'he_aac_v1'
								? [32000, 44100, 48000]
								: [
										7350, 8000, 11025, 12000, 16000, 22050, 24000, 32000, 44100, 48000, 64000,
										88200, 96000,
									],
					})),
					encoderType: 'faac' as const,
					bitrateKbpsMin: 1,
					bitrateKbpsMax: 1152,
					allowedModes: ['abr' as const, 'vbr' as const],
					defaultMode: { mode: 'abr' as const },
					explicitSampleRates: [
						7350, 8000, 11025, 12000, 16000, 22050, 24000, 32000, 44100, 48000, 64000, 88200, 96000,
					],
				},
			],
			nativeSpeedMax: 4,
			faacQualityPresets: [50, 100, 200],
			faacQualityDefault: 100,
			sampleRateAuto: true,
			explicitSampleRates: [
				7350, 8000, 11025, 12000, 16000, 22050, 24000, 32000, 44100, 48000, 64000, 88200, 96000,
			],
			channelOptions: ['auto' as const, 'mono' as const, 'stereo' as const],
		} satisfies GeneratedEncoderSettingsCapabilities,
		maxConcurrentJobs: {
			allowAuto: true,
			autoEffective: 4,
			fixedMin: 1,
			fixedMax: 8,
			fixedOptions: [1, 2, 3, 4, 5, 6, 7, 8],
		} satisfies GeneratedMaxConcurrentJobsCapabilities,
	} satisfies GeneratedRuntimeSettingsCapabilities;

	// Apply caller overrides on top of the generated-shape base.
	// Use a local mutable copy so we can apply overrides without
	// fighting the satisfies-narrowed types.
	const result: RuntimeSettingsCapabilities = { ...base };
	if (overrides.encoder?.availability) {
		result.encoder.availability = overrides.encoder.availability;
	}
	if (overrides.encoder) {
		const { availability: _, ...encoderOverrides } = overrides.encoder;
		Object.assign(result.encoder, encoderOverrides);
	}
	if (overrides.maxConcurrentJobs) {
		Object.assign(result.maxConcurrentJobs, overrides.maxConcurrentJobs);
	}

	return result;
}
