import type { AudioFile } from '../../types/audio';

const UNKNOWN_SAMPLE_RATE_HINT = 'Auto -> source audio';
const UNKNOWN_CHANNELS_HINT = 'Auto -> source audio';
const PARTIAL_SAMPLE_RATE_HINT = 'Auto -> mixed/unknown rates';
const PARTIAL_CHANNELS_HINT = 'Auto -> mixed/unknown channels';

type ResolutionState = 'same' | 'mixed' | 'partial' | 'unknown';

export type AutoResolutionHints = {
	sampleRateHint: string;
	channelsHint: string;
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

export const channelCountToLabel = (channels: number): string => {
	if (channels === 1) return 'Mono';
	if (channels === 2) return 'Stereo';
	return `${channels} ch`;
};

const formatChannelSummary = (channels: readonly number[]): string => {
	const uniqueChannels = [...new Set(channels)].sort((a, b) => a - b);
	if (uniqueChannels.length <= 3) {
		return uniqueChannels.map(channelCountToLabel).join('/');
	}
	return 'mixed channels';
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
	const knownChannels = getKnownValues(selectedFiles, (file) => file.channels);
	const state = resolveState(selectedFiles.length, knownChannels);
	if (state === 'unknown') return UNKNOWN_CHANNELS_HINT;
	if (state === 'partial') return PARTIAL_CHANNELS_HINT;
	if (state === 'mixed') return `Auto -> mixed (${formatChannelSummary(knownChannels)})`;
	return `Auto -> ${formatChannelSummary(knownChannels)}`;
};

export const resolveAutoResolutionHints = (
	selectedFiles: readonly AudioFile[],
): AutoResolutionHints => ({
	sampleRateHint: resolveSampleRateHint(selectedFiles),
	channelsHint: resolveChannelsHint(selectedFiles),
});
