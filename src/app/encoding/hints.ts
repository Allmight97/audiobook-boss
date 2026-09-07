import type { AudioFile } from '../../types/audio';

const UNKNOWN_SAMPLE_RATE_HINT = 'Auto -> source audio';
const UNKNOWN_CHANNELS_HINT = 'Auto -> source audio';
const PARTIAL_SAMPLE_RATE_HINT = 'Auto -> mixed/unknown rates';
const UNKNOWN_INPUT_CHANNELS_HINT = 'Unknown input channels: choose Mono or Stereo.';

type ResolutionState = 'same' | 'mixed' | 'partial' | 'unknown';

export type AutoResolutionHints = {
	sampleRateHint: string;
	channelsHint: string;
	hasMultichannelInput: boolean;
};

const toPositiveInt = (value: number | undefined): number | null => {
	if (typeof value !== 'number' || !Number.isFinite(value) || value <= 0) {
		return null;
	}
	return Math.round(value);
};

const getKnownValues = (
	selectedFiles: readonly AudioFile[],
	pickValue: (file: AudioFile) => number | undefined,
): number[] =>
	selectedFiles
		.map((file) => pickValue(file))
		.map(toPositiveInt)
		.filter((value): value is number => value !== null);

const resolveState = (selectedCount: number, knownValues: readonly number[]): ResolutionState => {
	if (selectedCount === 0 || knownValues.length === 0) {
		return 'unknown';
	}
	if (knownValues.length !== selectedCount) {
		return 'partial';
	}
	return new Set(knownValues).size === 1 ? 'same' : 'mixed';
};

const formatSampleRate = (sampleRate: number): string => {
	const kHz = sampleRate / 1000;
	return Number.isInteger(kHz) ? `${kHz}` : kHz.toFixed(2).replace(/0+$/, '').replace(/\.$/, '');
};

const formatSampleRateSummary = (sampleRates: readonly number[]): string => {
	const uniqueRates = [...new Set(sampleRates)].sort((a, b) => a - b);
	if (uniqueRates.length <= 3) {
		return `${uniqueRates.map(formatSampleRate).join('/')} kHz`;
	}
	const first = uniqueRates[0];
	const last = uniqueRates[uniqueRates.length - 1];
	return `${formatSampleRate(first)}-${formatSampleRate(last)} kHz`;
};

const resolveSampleRateHint = (selectedFiles: readonly AudioFile[]): string => {
	const knownSampleRates = getKnownValues(selectedFiles, (file) => file.sampleRate);
	const state = resolveState(selectedFiles.length, knownSampleRates);
	if (state === 'unknown') return UNKNOWN_SAMPLE_RATE_HINT;
	if (state === 'partial') return PARTIAL_SAMPLE_RATE_HINT;
	if (state === 'mixed') return `Auto -> mixed (${formatSampleRateSummary(knownSampleRates)})`;
	return `Auto -> ${formatSampleRateSummary(knownSampleRates)}`;
};

const resolveChannelsHint = (selectedFiles: readonly AudioFile[]): string => {
	const validFiles = selectedFiles.filter((file) => file.isValid);
	if (validFiles.length === 0) return UNKNOWN_CHANNELS_HINT;
	const knownChannels = getKnownValues(validFiles, (file) => file.channels);
	if (knownChannels.some((channels) => channels > 2)) {
		return 'Multichannel input: choose Mono or Stereo to downmix.';
	}
	if (knownChannels.length !== validFiles.length) return UNKNOWN_INPUT_CHANNELS_HINT;
	if (knownChannels.includes(1) && knownChannels.includes(2)) {
		return 'Auto -> source channels; Stereo when merged';
	}
	return knownChannels.includes(2) ? 'Auto -> Stereo' : 'Auto -> Mono';
};

export const resolveAutoResolutionHints = (
	selectedFiles: readonly AudioFile[],
): AutoResolutionHints => ({
	sampleRateHint: resolveSampleRateHint(selectedFiles),
	channelsHint: resolveChannelsHint(selectedFiles),
	hasMultichannelInput: selectedFiles.some((file) => file.isValid && (file.channels ?? 0) > 2),
});
