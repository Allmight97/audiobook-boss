import type {
	AudioChoice,
	AudioChoiceFacts,
	AudioEdit,
	FaacRateControl,
	TitlePlan,
} from '../../types/session';
import type {
	AudiobookFormat,
	AudioIntent,
	EncoderSettingsCapabilities,
	EncoderType,
	FaacProfile,
} from '../../types/audio';

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
	/** A note under the sample rate, when the engine has one. */
	readonly sampleRateHint: string | null;
	readonly channels: string;
	readonly channelOptions: ReadonlyArray<EncodingOption>;
	readonly channelsDisabled: boolean;
	readonly channelsHint: string | null;
};

/** What the panel shows for one engine choice. */
export type EncodingSource = {
	readonly choice: AudioChoice;
	readonly facts: AudioChoiceFacts;
	readonly capabilities: EncoderSettingsCapabilities | null;
	/** The engine's plan for each title shown; `null` for the defaults. */
	readonly plans: readonly TitlePlan[] | null;
	/** Whether a shown source has more than two channels. */
	readonly multichannelInput: boolean;
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

/**
 * What Auto resolves to, from the engine's plans: one value when every title
 * resolved to it, "Differs by title" when they disagree, and no detail while
 * a plan is pending or failed. The defaults describe future imports.
 */
function autoLabel(
	plans: readonly TitlePlan[] | null,
	auto: boolean,
	value: (plan: Extract<TitlePlan, { kind: 'resolved' }>['plan']) => string,
): string {
	if (plans === null) return 'Auto · Source audio';
	if (!auto || plans.length === 0 || plans.some((plan) => plan.kind !== 'resolved')) return 'Auto';
	const values = new Set(plans.map((plan) => (plan.kind === 'resolved' ? value(plan.plan) : '')));
	return values.size === 1 ? `Auto · ${[...values][0]}` : 'Auto · Differs by title';
}

function kHz(sampleRate: number): string {
	return `${sampleRate / 1000} kHz`;
}

function channelCount(channels: number): string {
	return channels === 1 ? 'Mono' : channels === 2 ? 'Stereo' : `${channels} channels`;
}

/** The engine's reason a shown title failed on `field`. */
function planProblem(
	plans: readonly TitlePlan[] | null,
	field: 'sampleRate' | 'channels',
): string | null {
	for (const plan of plans ?? []) {
		if (plan.kind === 'failed' && plan.field === field) return plan.message;
	}
	return null;
}

function sampleRateHint(source: EncodingSource): string | null {
	const { choice, facts, capabilities } = source;
	if (choice.sampleRate !== 'auto' && capabilities && !facts.sampleRateSupported)
		return 'Choose a supported sample rate for this encoder.';
	return planProblem(source.plans, 'sampleRate');
}

function channelsHint(source: EncodingSource): string | null {
	const problem = planProblem(source.plans, 'channels');
	if (problem) return problem;
	return source.choice.channels !== 'auto' && source.multichannelInput
		? 'Surround downmix omits bass effects (LFE).'
		: null;
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
						? autoLabel(source.plans, choice.sampleRate === 'auto', (plan) => kHz(plan.sampleRate))
						: sampleRateLabel(value),
				disabled: value !== 'auto' && !facts.allowedSampleRates.includes(Number(value)),
			}))
		: [];
	const channelOptions = (capabilities?.channelOptions ?? []).map((value) => ({
		value,
		label:
			value === 'auto'
				? autoLabel(source.plans, choice.channels === 'auto', (plan) => channelCount(plan.channels))
				: channelLabel(value),
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
		sampleRateHint: sampleRateHint(source),
		channels: choice.channels,
		channelOptions,
		channelsDisabled: channelOptions.length === 0,
		channelsHint: channelsHint(source),
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
