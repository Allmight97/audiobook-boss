import type { EncoderSettings } from '../../types/audio';

// FDK's encoder guide gives rough stereo averages for these profile/quality pairs.
// They are content-dependent reference values, not bitrate targets or ceilings.
// https://github.com/mstorsjo/fdk-aac/blob/v2.0.3/libAACenc/include/aacenc_lib.h
const FDK_STEREO_REFERENCE: Record<string, readonly number[]> = {
	he_aac_v2: [32],
	he_aac_v1: [0, 72],
	aac_lc: [0, 0, 112, 148, 228],
};

export function estimateKbpsFromSettings(
	settings: EncoderSettings,
	sampleRate?: number,
): number | null {
	const { bitrateMode, bitrateKbps } = settings;
	if (bitrateMode.mode !== 'vbr') return bitrateKbps;
	if (
		settings.encoderType !== 'fdk_he_aac' ||
		settings.channels !== 'stereo' ||
		(sampleRate !== 44100 && sampleRate !== 48000)
	)
		return null;
	return FDK_STEREO_REFERENCE[settings.fdkProfile ?? 'auto']?.[bitrateMode.value - 1] || null;
}
