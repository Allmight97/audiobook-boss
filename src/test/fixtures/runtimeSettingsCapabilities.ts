import type { EncoderAvailability, RuntimeSettingsCapabilities } from '../../types/audio';

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
	const availability = overrides.encoder?.availability ?? encoderAvailabilityFixture();
	return {
		encoder: {
			availability,
			encoderTypes: ['auto', 'fdk_he_aac', 'aac_at', 'native_aac'],
			autoResolutionOrder: ['fdk_he_aac', 'aac_at', 'native_aac'],
			bitrateKbpsOptions: [48, 56, 64, 72, 80, 88, 96, 104, 112, 120, 128],
			bitrateModesByEncoder: [
				{
					encoderType: 'auto',
					allowedModes: ['vbr'],
					defaultMode: { mode: 'vbr', value: 3 },
				},
				{
					encoderType: 'fdk_he_aac',
					allowedModes: ['vbr'],
					defaultMode: { mode: 'vbr', value: 3 },
				},
				{ encoderType: 'aac_at', allowedModes: ['cvbr'], defaultMode: { mode: 'cvbr' } },
				{ encoderType: 'native_aac', allowedModes: ['cbr'], defaultMode: { mode: 'cbr' } },
			],
			vbrLevelMin: 1,
			vbrLevelMax: 5,
			vbrLevelDefault: 3,
			threadFixedMin: 1,
			threadFixedMax: 1024,
			sampleRateAuto: true,
			explicitSampleRates: [22050, 32000, 44100, 48000],
			channelOptions: ['auto', 'mono', 'stereo'],
			...overrides.encoder,
		},
		maxConcurrentJobs: {
			allowAuto: true,
			autoEffective: 4,
			fixedMin: 1,
			fixedMax: 8,
			fixedOptions: [1, 2, 3, 4, 5, 6, 7, 8],
			...overrides.maxConcurrentJobs,
		},
	};
}
