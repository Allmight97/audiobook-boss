import type { AudioFile } from '../../types/audio';
import { resetAutoHints, setChannelsAutoHint, setSampleRateAutoHint } from './state.svelte';

const UNKNOWN_SAMPLE_RATE_HINT = 'Auto resolves from source audio.';
const UNKNOWN_CHANNELS_HINT = 'Auto resolves from source audio.';
const MIXED_SAMPLE_RATE_HINT = 'Auto resolves per file (mixed inputs).';
const MIXED_CHANNELS_HINT = 'Auto resolves per file (mixed inputs).';

type ResolutionState = 'single-exact' | 'multi-same' | 'mixed' | 'unknown';

export interface AutoResolutionHints {
	sampleRateHint: string;
	channelsHint: string;
}

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
		.map((file) => toPositiveInt(pickValue(file)))
		.filter((value): value is number => value !== null);

const resolveState = (selectedCount: number, knownValues: readonly number[]): ResolutionState => {
	if (selectedCount === 0 || knownValues.length === 0) {
		return 'unknown';
	}

	if (selectedCount === 1) {
		return 'single-exact';
	}

	const uniqueCount = new Set(knownValues).size;
	if (knownValues.length === selectedCount && uniqueCount === 1) {
		return 'multi-same';
	}

	return 'mixed';
};

export const channelCountToLabel = (channels: number): string => {
	if (channels === 1) return 'Mono';
	if (channels === 2) return 'Stereo';
	return `${channels} ch`;
};

const resolveSampleRateHint = (selectedFiles: readonly AudioFile[]): string => {
	const selectedCount = selectedFiles.length;
	const knownSampleRates = getKnownValues(selectedFiles, (file) => file.sampleRate);
	const state = resolveState(selectedCount, knownSampleRates);

	if (state === 'unknown') {
		return UNKNOWN_SAMPLE_RATE_HINT;
	}

	if (state === 'mixed') {
		return MIXED_SAMPLE_RATE_HINT;
	}

	const sampleRate = knownSampleRates[0];
	if (state === 'single-exact') {
		return `Auto resolves to ${sampleRate} Hz from selected file.`;
	}

	return `Auto resolves to ${sampleRate} Hz across selected files.`;
};

const resolveChannelsHint = (selectedFiles: readonly AudioFile[]): string => {
	const selectedCount = selectedFiles.length;
	const knownChannels = getKnownValues(selectedFiles, (file) => file.channels);
	const state = resolveState(selectedCount, knownChannels);

	if (state === 'unknown') {
		return UNKNOWN_CHANNELS_HINT;
	}

	if (state === 'mixed') {
		return MIXED_CHANNELS_HINT;
	}

	const channelLabel = channelCountToLabel(knownChannels[0]);
	if (state === 'single-exact') {
		return `Auto resolves to ${channelLabel} from selected file.`;
	}

	return `Auto resolves to ${channelLabel} across selected files.`;
};

export const resolveAutoResolutionHints = (
	selectedFiles: readonly AudioFile[],
): AutoResolutionHints => ({
	sampleRateHint: resolveSampleRateHint(selectedFiles),
	channelsHint: resolveChannelsHint(selectedFiles),
});

export const renderAutoResolutionHints = (selectedFiles: readonly AudioFile[]): void => {
	const hints = resolveAutoResolutionHints(selectedFiles);
	setSampleRateAutoHint(hints.sampleRateHint);
	setChannelsAutoHint(hints.channelsHint);
};

export const resetAutoResolutionHints = (): void => {
	resetAutoHints();
};
