import type { AudioChoice, AudioChoiceFacts, AudioEdit, AutoReading } from '../../types/session';
import type {
	AudiobookFormat,
	AudioIntent,
	EncoderSettingsCapabilities,
	EncoderType,
	FaacProfile,
} from '../../types/audio';

export type EncodingField = AudioEdit['field'];

type EncodingOption = {
	readonly value: string;
	readonly label: string;
	readonly disabled?: boolean;
};

export type EncodingView = {
	readonly format: AudiobookFormat;
	readonly intent: AudioIntent;
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
	readonly sampleRateHint: string | null;
	readonly channels: string;
	readonly channelOptions: ReadonlyArray<EncodingOption>;
	readonly channelsHint: string | null;
};

export type EncodingSource = {
	readonly choice: AudioChoice;
	readonly facts: AudioChoiceFacts;
	readonly capabilities: EncoderSettingsCapabilities | null;
	readonly auto: EncodingAuto | null;
};

export type EncodingAuto = {
	readonly sampleRate: AutoReading;
	readonly channels: AutoReading;
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

function qualityLabel(capabilities: EncoderSettingsCapabilities, value: number): string {
	const standard = capabilities.faacQualityDefault;
	const label = value === standard ? 'Standard' : value < standard ? 'Smaller' : 'Higher';
	return `${label} (${value})`;
}

function sampleRateLabel(value: string): string {
	return `${value} Hz`;
}

function channelLabel(value: string): string {
	return value === 'mono' ? 'Mono' : 'Stereo';
}

function autoLabel(reading: AutoReading | undefined, value: (count: number) => string): string {
	if (reading === undefined) return 'Auto · Source audio';
	switch (reading.outcome.kind) {
		case 'value':
			return `Auto · ${value(reading.outcome.value)}`;
		case 'differs':
			return 'Auto · Differs by title';
		case 'unresolved':
			return 'Auto';
	}
}

function kHz(sampleRate: number): string {
	return `${sampleRate / 1000} kHz`;
}

function channelCount(channels: number): string {
	return channels === 1 ? 'Mono' : channels === 2 ? 'Stereo' : `${channels} channels`;
}

function sampleRateHint(source: EncodingSource): string | null {
	const { choice, facts, capabilities } = source;
	if (choice.sampleRate !== 'auto' && capabilities && !facts.sampleRateSupported)
		return 'Choose a supported sample rate for this encoder.';
	return source.auto?.sampleRate.problem ?? null;
}

function channelsHint(source: EncodingSource): string | null {
	const problem = source.auto?.channels.problem;
	if (problem) return problem;
	return source.facts.downmixWarning ? 'Surround downmix omits bass effects (LFE).' : null;
}

export function projectView(source: EncodingSource): EncodingView {
	const { choice, facts, capabilities } = source;
	const isOpus = facts.effectiveEncoder === 'opus';
	const sampleRate = choice.sampleRate === 'auto' ? 'auto' : String(choice.sampleRate.explicit);
	const flavorOptions: EncodingOption[] = facts.encoderOptions.map((option) => ({
		value: option.encoder,
		label: encoderLabel(option.encoder),
		disabled: !option.available,
	}));
	const sampleRateOptions = capabilities
		? [
				...(capabilities.sampleRateAuto ? ['auto'] : []),
				...capabilities.explicitSampleRates.map(String),
			].map((value) => ({
				value,
				label: value === 'auto' ? autoLabel(source.auto?.sampleRate, kHz) : sampleRateLabel(value),
				disabled: value !== 'auto' && !facts.allowedSampleRates.includes(Number(value)),
			}))
		: [];
	const channelOptions = (capabilities?.channelOptions ?? []).map((value) => ({
		value,
		label: value === 'auto' ? autoLabel(source.auto?.channels, channelCount) : channelLabel(value),
	}));
	const showQuality = facts.bitrateMode.mode === 'vbr';

	return {
		format: choice.format,
		intent: choice.intent,
		effectiveFlavor: facts.effectiveEncoder,
		flavorOptions,
		flavorDisabled: facts.encoderLocked,
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
					: value === capabilities?.nativeSpeedMax
						? `${value} · Fastest search`
						: String(value),
		})),
		showQuality,
		quality: choice.faacQuality,
		qualityOptions: capabilities
			? capabilities.faacQualityPresets.map((value) => ({
					value: String(value),
					label: qualityLabel(capabilities, value),
				}))
			: [],
		bitrate: isOpus ? choice.opusBitrateKbps : choice.aacBitrateKbps,
		bitrateKbpsMin: facts.bitrateKbpsMin,
		sampleRate,
		sampleRateOptions,
		sampleRateHint: sampleRateHint(source),
		channels: choice.channels,
		channelOptions,
		channelsHint: channelsHint(source),
	};
}

function oneOf<T extends string>(options: readonly T[], value: string): value is T {
	return (options as readonly string[]).includes(value);
}

export function editFor(field: EncodingField, value: string): AudioEdit | null {
	const number = Number(value);
	const whole = value.trim() !== '' && Number.isInteger(number) && number >= 0;
	switch (field) {
		case 'format':
			return oneOf(['m4b', 'm4aOpus', 'mkaOpus'] as const, value)
				? { field: 'format', value }
				: null;
		case 'intent':
			return oneOf(['auto', 'encode', 'preserve'] as const, value)
				? { field: 'intent', value }
				: null;
		case 'encoder':
			return oneOf(['auto', 'aac_at', 'native_aac', 'faac'] as const, value)
				? { field: 'encoder', value }
				: null;
		case 'faacProfile':
			return oneOf(['auto', 'aac_lc', 'he_aac_v1'] as const, value)
				? { field: 'faacProfile', value }
				: null;
		case 'rateControl':
			return value === 'abr' || value === 'vbr' ? { field: 'rateControl', value: value } : null;
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
			return oneOf(['auto', 'mono', 'stereo'] as const, value)
				? { field: 'channels', value }
				: null;
	}
}
