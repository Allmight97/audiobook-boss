import type {
	AudioChoice,
	AudioChoiceFacts,
	AudioEdit,
	FaacRateControl,
} from '../../types/session';
import type {
	AudiobookFormat,
	AudioIntent,
	EncoderSettingsCapabilities,
	EncoderType,
	FaacProfile,
} from '../../types/audio';
import type { AutoResolutionHints } from './hints';

export type EncodingField =
	| 'format'
	| 'intent'
	| 'encoder'
	| 'quality'
	| 'faacProfile'
	| 'rateControl'
	| 'nativeSpeed'
	| 'bitrate'
	| 'sampleRate'
	| 'channels';

export type EncodingOption = {
	readonly value: string;
	readonly label: string;
	readonly disabled?: boolean;
};

export type EncodingView = {
	readonly format: AudiobookFormat;
	readonly intent: AudioIntent;
	readonly flavor: string;
	readonly effectiveFlavor: string;
	readonly flavorOptions: ReadonlyArray<EncodingOption>;
	readonly flavorDisabled: boolean;
	readonly profileDisplay: string;
	readonly faac: boolean;
	readonly faacProfile: FaacProfile;
	readonly faacProfileOptions: ReadonlyArray<EncodingOption>;
	readonly rateControl: string;
	readonly rateControlOptions: ReadonlyArray<EncodingOption>;
	readonly qualityBitrateLabel: 'Quality' | 'Bitrate (kbps)';
	readonly native: boolean;
	readonly bitrateKbpsMax: number;
	readonly nativeSpeed: number;
	readonly nativeSpeedOptions: ReadonlyArray<EncodingOption>;
	readonly showQuality: boolean;
	readonly quality: number;
	readonly qualityOptions: ReadonlyArray<EncodingOption>;
	readonly bitrate: number;
	readonly bitrateKbpsMin: number;
	readonly sampleRate: string;
	readonly sampleRateOptions: ReadonlyArray<EncodingOption>;
	readonly sampleRateDisabled: boolean;
	readonly sampleRateHint: string;
	readonly channels: string;
	readonly channelOptions: ReadonlyArray<EncodingOption>;
	readonly channelsDisabled: boolean;
	readonly channelsHint: string;
};

/** What the panel shows for one engine choice. */
export type EncodingSource = {
	readonly choice: AudioChoice;
	readonly facts: AudioChoiceFacts;
	readonly capabilities: EncoderSettingsCapabilities | null;
	readonly hints: AutoResolutionHints;
};

const ENCODER_PROFILES: Record<EncoderType, string> = {
	auto: 'AAC-LC',
	faac: 'AAC',
	aac_at: 'AAC-LC',
	native_aac: 'AAC-LC',
	opus: 'Opus',
};

const FAAC_PROFILES: Record<FaacProfile, string> = {
	auto: 'Auto · FAAC chooses LC or HE',
	aac_lc: 'AAC-LC',
	he_aac_v1: 'HE-AAC v1',
};

const opus = (format: AudiobookFormat) => format === 'm4aOpus' || format === 'mkaOpus';

function rangeOptions(min: number, max: number): number[] {
	const values: number[] = [];
	for (let value = min; value <= max; value += 1) values.push(value);
	return values;
}

function encoderLabel(value: EncoderType): string {
	switch (value) {
		case 'aac_at':
			return 'Apple AAC';
		case 'native_aac':
			return 'Native AAC (NMR)';
		case 'faac':
			return 'FAAC';
		case 'opus':
			return 'Opus (libopus)';
		default:
			return 'App default';
	}
}

function qualityLabel(capabilities: EncoderSettingsCapabilities | null, value: number): string {
	const standard = capabilities?.faacQualityDefault ?? 100;
	const label = value === standard ? 'Standard' : value < standard ? 'Smaller' : 'Higher';
	return `${label} (${value})`;
}

function sampleRateLabel(value: string): string {
	return value === 'auto' ? 'Auto' : `${value} Hz`;
}

function channelLabel(value: string): string {
	switch (value) {
		case 'auto':
			return 'Auto';
		case 'mono':
			return 'Mono';
		case 'stereo':
			return 'Stereo';
		default:
			return value;
	}
}

function sampleRateText(source: EncodingSource): string {
	const { choice, facts, capabilities } = source;
	if (choice.sampleRate === 'auto')
		return opus(choice.format)
			? 'Auto → next supported Opus input rate'
			: source.hints.sampleRateHint;
	if (capabilities && !facts.sampleRateSupported)
		return 'Choose a supported sample rate for this encoder.';
	return `Using ${sampleRateLabel(String(choice.sampleRate.explicit))}.`;
}

function channelsText(source: EncodingSource): string {
	const { choice, hints } = source;
	if (choice.channels === 'auto') return hints.channelsHint;
	const downmix = hints.hasMultichannelInput ? ' Surround downmix omits bass effects (LFE).' : '';
	return `Using ${channelLabel(choice.channels)}.${downmix}`;
}

function autoResolutionLabel(hint: string): string {
	const detail = hint.replace(/^Auto\s*(?:->|→)\s*/, '');
	return detail === hint
		? 'Auto · Choose channels'
		: `Auto · ${detail.charAt(0).toUpperCase()}${detail.slice(1)}`;
}

function encoderUnavailable(
	capabilities: EncoderSettingsCapabilities | null,
	encoder: EncoderType,
): boolean {
	const availability = capabilities?.availability;
	if (!availability) return false;
	if (encoder === 'aac_at') return !availability.aacAtAvailable;
	if (encoder === 'native_aac') return !availability.nativeAacAvailable;
	return false;
}

export function projectView(source: EncodingSource): EncodingView {
	const { choice, facts, capabilities } = source;
	const isOpus = opus(choice.format);
	const sampleRate = choice.sampleRate === 'auto' ? 'auto' : String(choice.sampleRate.explicit);
	const flavorOptions: EncodingOption[] =
		capabilities === null
			? [{ value: 'auto', label: 'Loading…', disabled: true }]
			: capabilities.encoderTypes
					.filter((flavor) => (isOpus ? flavor === 'opus' : flavor !== 'opus' && flavor !== 'auto'))
					.map((flavor) => ({
						value: flavor,
						label: encoderLabel(flavor),
						disabled: encoderUnavailable(capabilities, flavor),
					}));
	const sampleRateOptions = capabilities
		? [
				...(capabilities.sampleRateAuto ? ['auto'] : []),
				...capabilities.explicitSampleRates.map(String),
			].map((value) => ({
				value,
				label:
					value === 'auto'
						? autoResolutionLabel(
								sampleRateText({ ...source, choice: { ...choice, sampleRate: 'auto' } }),
							)
						: sampleRateLabel(value),
				disabled: value !== 'auto' && !facts.allowedSampleRates.includes(Number(value)),
			}))
		: [];
	const channelOptions = (capabilities?.channelOptions ?? []).map((value) => ({
		value,
		label: value === 'auto' ? autoResolutionLabel(source.hints.channelsHint) : channelLabel(value),
	}));
	const showQuality = facts.bitrateMode.mode === 'vbr';

	return {
		format: choice.format,
		intent: choice.intent,
		flavor: isOpus ? 'opus' : choice.encoder,
		effectiveFlavor: facts.effectiveEncoder,
		flavorOptions,
		flavorDisabled: isOpus || capabilities === null,
		profileDisplay: ENCODER_PROFILES[facts.effectiveEncoder],
		qualityBitrateLabel: showQuality ? 'Quality' : 'Bitrate (kbps)',
		native: facts.effectiveEncoder === 'native_aac',
		faac: !isOpus && choice.encoder === 'faac',
		faacProfile: choice.faacProfile,
		faacProfileOptions: facts.faacProfiles.map((profile) => ({
			value: profile,
			label: FAAC_PROFILES[profile],
		})),
		rateControl: choice.faacRateControl,
		rateControlOptions: facts.allowedModes.map((mode) => ({
			value: mode,
			label: mode === 'abr' ? 'Average bitrate (ABR)' : 'Quality (VBR)',
		})),
		bitrateKbpsMax: facts.bitrateKbpsMax,
		nativeSpeed: choice.nativeSpeed,
		nativeSpeedOptions: rangeOptions(0, capabilities?.nativeSpeedMax ?? 0).map((value) => ({
			value: String(value),
			label:
				value === 0
					? '0 · Full search (default)'
					: value === 4
						? '4 · Fastest search'
						: String(value),
		})),
		showQuality,
		quality: choice.faacQuality,
		qualityOptions: (capabilities?.faacQualityPresets ?? []).map((value) => ({
			value: String(value),
			label: qualityLabel(capabilities, value),
		})),
		bitrate: isOpus ? choice.opusBitrateKbps : choice.aacBitrateKbps,
		bitrateKbpsMin: facts.bitrateKbpsMin,
		sampleRate,
		sampleRateOptions,
		sampleRateDisabled: sampleRateOptions.length === 0,
		sampleRateHint: sampleRateText(source),
		channels: choice.channels,
		channelOptions,
		channelsDisabled: channelOptions.length === 0,
		channelsHint: channelsText(source),
	};
}

/** The engine edit a control's value names, or `null` for a value it cannot name. */
export function editFor(field: EncodingField, value: string): AudioEdit | null {
	const number = Number(value);
	const whole = value.trim() !== '' && Number.isInteger(number) && number >= 0;
	switch (field) {
		case 'format':
			return ['m4b', 'mp3', 'm4aOpus', 'mkaOpus'].includes(value)
				? { field: 'format', value: value as AudiobookFormat }
				: null;
		case 'intent':
			return ['auto', 'encode', 'preserve'].includes(value)
				? { field: 'intent', value: value as AudioIntent }
				: null;
		case 'encoder':
			return ['auto', 'aac_at', 'native_aac', 'faac'].includes(value)
				? { field: 'encoder', value: value as EncoderType }
				: null;
		case 'faacProfile':
			return ['auto', 'aac_lc', 'he_aac_v1'].includes(value)
				? { field: 'faacProfile', value: value as FaacProfile }
				: null;
		case 'rateControl':
			return value === 'abr' || value === 'vbr'
				? { field: 'rateControl', value: value as FaacRateControl }
				: null;
		case 'quality':
			return whole ? { field: 'quality', value: number } : null;
		case 'nativeSpeed':
			return whole ? { field: 'nativeSpeed', value: number } : null;
		case 'bitrate':
			return whole ? { field: 'bitrate', value: number } : null;
		case 'sampleRate':
			if (value === 'auto') return { field: 'sampleRate', value: 'auto' };
			return whole ? { field: 'sampleRate', value: { explicit: number } } : null;
		case 'channels':
			return ['auto', 'mono', 'stereo'].includes(value)
				? { field: 'channels', value: value as 'auto' | 'mono' | 'stereo' }
				: null;
	}
}
