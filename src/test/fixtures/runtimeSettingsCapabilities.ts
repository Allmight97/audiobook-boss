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
	const fdkAvailable = overrides.fdkAvailable ?? true;
	const aacAtAvailable = overrides.aacAtAvailable ?? true;
	const nativeAacAvailable = overrides.nativeAacAvailable ?? true;
	return {
		fdkAvailable,
		aacAtAvailable,
		nativeAacAvailable,
		fdkSource: fdkAvailable ? 'detected' : 'none',
		autoEncoder: fdkAvailable ? 'fdk_he_aac' : aacAtAvailable ? 'aac_at' : 'native_aac',
		detectedToolchainPath: fdkAvailable ? '/opt/homebrew/bin/ffmpeg' : undefined,
		statusMessage: fdkAvailable
			? 'FDK AAC detected and ready.'
			: 'No external FFmpeg toolchain with libfdk_aac was detected.',
		...overrides,
	};
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
				fdkAvailable: true,
				aacAtAvailable: true,
				nativeAacAvailable: true,
				fdkSource: 'detected' as const,
				autoEncoder: 'fdk_he_aac' as const,
				detectedToolchainPath: '/opt/homebrew/bin/ffmpeg',
				statusMessage: 'FDK AAC detected and ready.',
			} satisfies GeneratedEncoderAvailability,
			encoderTypes: ['auto', 'fdk_he_aac', 'aac_at', 'native_aac'],
			autoResolutionOrder: ['fdk_he_aac', 'aac_at', 'native_aac'],
			bitrateKbpsOptions: [48, 56, 64, 72, 80, 88, 96, 104, 112, 120, 128],
			bitrateModesByEncoder: [
				{
					encoderType: 'auto' as const,
					allowedModes: ['vbr' as const],
					defaultMode: { mode: 'vbr' as const, value: 3 },
				},
				{
					encoderType: 'fdk_he_aac' as const,
					allowedModes: ['vbr' as const],
					defaultMode: { mode: 'vbr' as const, value: 3 },
				},
				{ encoderType: 'aac_at' as const, allowedModes: ['cvbr' as const], defaultMode: { mode: 'cvbr' as const } },
				{ encoderType: 'native_aac' as const, allowedModes: ['cbr' as const], defaultMode: { mode: 'cbr' as const } },
			],
			vbrLevelMin: 1,
			vbrLevelMax: 5,
			vbrLevelDefault: 3,
			threadFixedMin: 1,
			threadFixedMax: 1024,
			sampleRateAuto: true,
			explicitSampleRates: [22050, 32000, 44100, 48000],
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
