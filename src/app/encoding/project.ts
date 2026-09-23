import type { EncoderDefaults } from '../../types/appSettings';
import type {
	AudiobookFormat,
	AudioIntent,
	BitrateMode,
	EncoderAvailability,
	EncoderChannelConfig,
	EncoderConfigurationCapability,
	EncoderSettingsCapabilities,
	EncoderType,
	FaacProfile,
	EncodingRequestConfig,
	SampleRateConfig,
} from '../../types/audio';
import { defaultEncoderSettings } from '../../types/audio';
import { estimateKbpsFromRequest } from './estimate';

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
	readonly flavorOptions: ReadonlyArray<EncodingOption>;
	readonly flavorDisabled: boolean;
	readonly availabilityHint: string;
	readonly fdkSetupNeeded: boolean;
	readonly profileDisplay: string;
	readonly faac: boolean;
	readonly faacProfile: FaacProfile;
	readonly faacProfileOptions: ReadonlyArray<EncodingOption>;
	readonly rateControl: string;
	readonly rateControlOptions: ReadonlyArray<EncodingOption>;
	readonly profileHint: string;
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
	readonly estimatedBitrateText: string;
	readonly sampleRate: string;
	readonly sampleRateOptions: ReadonlyArray<EncodingOption>;
	readonly sampleRateDisabled: boolean;
	readonly sampleRateHint: string;
	readonly channels: string;
	readonly channelOptions: ReadonlyArray<EncodingOption>;
	readonly channelsDisabled: boolean;
	readonly channelsHint: string;
	readonly afterburner: boolean;
};

export type EncodingBag = {
	format: AudiobookFormat;
	intent: AudioIntent;
	opusBitrate: number;
	flavor: EncoderType;
	hydratedBitrateMode: BitrateMode;
	faacProfile: FaacProfile;
	faacMode: 'abr' | 'vbr';
	faacQuality: number;
	quality: number;
	nativeSpeed: number;
	bitrate: number;
	sampleRate: string;
	channels: EncoderChannelConfig;
	afterburner: boolean;
	capabilities: EncoderSettingsCapabilities | null;
	availability: EncoderAvailability | null;
	sampleRateHint: string;
	channelsHint: string;
	hasMultichannelInput: boolean;
};

const DEFAULT_SAMPLE_RATE_HINT = 'Auto -> source audio';
const DEFAULT_CHANNELS_HINT = 'Auto -> source audio';
const DEFAULT_AVAILABILITY_HINT = 'Checking encoder availability…';
const NATIVE_AAC_HINT = 'NMR AAC-LC. Preview to compare settings on your audio.';
const ENCODER_PROFILES: Record<Exclude<EncoderType, 'faac'>, string> = {
	auto: 'HE-AAC v1',
	fdk_he_aac: 'HE-AAC v1',
	aac_at: 'AAC-LC',
	native_aac: 'AAC-LC',
	opus: 'Opus',
};

const FAAC_PROFILES: Record<FaacProfile, string> = {
	auto: 'Auto · let FAAC choose',
	aac_lc: 'AAC-LC',
	he_aac_v1: 'HE-AAC v1',
};

export function createDefaultBag(): EncodingBag {
	return {
		format: 'm4b',
		intent: 'auto',
		opusBitrate: 64,
		flavor: 'auto',
		hydratedBitrateMode: defaultEncoderSettings().bitrateMode,
		faacProfile: 'auto',
		faacMode: 'abr',
		faacQuality: 100,
		quality: 3,
		nativeSpeed: 0,
		bitrate: 64,
		sampleRate: 'auto',
		channels: 'auto',
		afterburner: true,
		capabilities: null,
		availability: null,
		sampleRateHint: DEFAULT_SAMPLE_RATE_HINT,
		channelsHint: DEFAULT_CHANNELS_HINT,
		hasMultichannelInput: false,
	};
}

function rangeOptions(min: number, max: number): number[] {
	const values: number[] = [];
	for (let value = min; value <= max; value += 1) {
		values.push(value);
	}
	return values;
}

function encoderFlavorLabel(flavor: EncoderType): string {
	switch (flavor) {
		case 'fdk_he_aac':
			return 'FDK AAC';
		case 'aac_at':
			return 'Apple AAC';
		case 'native_aac':
			return 'Native AAC (NMR)';
		case 'faac':
			return 'FAAC';
		case 'opus':
			return 'Opus';
		default:
			return 'App default';
	}
}

function compactToolchainPath(path: string | null | undefined): string | null {
	if (!path) return null;
	if (path.length <= 38) return path;
	const normalized = path.replace(/\\/g, '/');
	const parts = normalized.split('/').filter(Boolean);
	const parent = parts[parts.length - 2];
	const filename = parts[parts.length - 1];
	if (!parent || !filename) return path;
	if (normalized.startsWith('/opt/homebrew/')) {
		return `/opt/homebrew/.../${parent}/${filename}`;
	}
	if (normalized.startsWith('/usr/local/')) {
		return `/usr/local/.../${parent}/${filename}`;
	}
	const root = normalized.startsWith('/') ? `/${parts[0]}` : (parts[0] ?? '');
	return root ? `${root}/.../${parent}/${filename}` : `.../${parent}/${filename}`;
}

const opus = (bag: EncodingBag) => bag.format === 'm4aOpus' || bag.format === 'mkaOpus';
function effectiveEncoder(bag: EncodingBag): EncoderType {
	if (opus(bag)) return 'opus';
	if (bag.flavor !== 'auto') return bag.flavor;
	return bag.availability?.autoEncoder ?? 'auto';
}

function disabledEncoderOptions(availability: EncoderAvailability | null) {
	return {
		fdk_he_aac: !availability?.fdkAvailable,
		aac_at: availability ? !availability.aacAtAvailable : false,
		native_aac: availability ? !availability.nativeAacAvailable : false,
	};
}

function flavorIsUnavailable(bag: EncodingBag): boolean {
	if (!bag.availability || bag.flavor === 'auto') return false;
	const disabled = disabledEncoderOptions(bag.availability);
	if (bag.flavor === 'fdk_he_aac') return disabled.fdk_he_aac;
	if (bag.flavor === 'aac_at') return disabled.aac_at;
	if (bag.flavor === 'native_aac') return disabled.native_aac;
	return false;
}

function sampleRateFromBag(bag: EncodingBag): SampleRateConfig {
	if (bag.sampleRate === 'auto') return 'auto';
	const parsed = Number.parseInt(bag.sampleRate, 10);
	if (!Number.isFinite(parsed)) return 'auto';
	if (!bag.capabilities) return { explicit: parsed };
	return bag.capabilities.explicitSampleRates.includes(parsed) ? { explicit: parsed } : 'auto';
}

function bitrateModeFromBag(bag: EncodingBag): BitrateMode {
	if (opus(bag)) return { mode: 'vbr_target' };
	if (bag.flavor === 'faac')
		return bag.faacMode === 'vbr' ? { mode: 'vbr', value: bag.faacQuality } : { mode: 'abr' };
	const mode = encoderConfiguration(bag)?.defaultMode ?? bag.hydratedBitrateMode;
	return mode.mode === 'vbr' ? { mode: 'vbr', value: bag.quality } : mode;
}

export function bagRequest(bag: EncodingBag): EncodingRequestConfig {
	return {
		encoderSettings: {
			encoderType: opus(bag) ? 'opus' : bag.flavor,
			channels: bag.channels,
			bitrateKbps: opus(bag) ? bag.opusBitrate : bag.bitrate,
			bitrateMode: bitrateModeFromBag(bag),
			afterburner: bag.afterburner,
			nativeAacSpeed: bag.nativeSpeed,
			faacProfile: bag.faacProfile,
		},
		sampleRate: sampleRateFromBag(bag),
	};
}

export function bagDefaults(bag: EncodingBag): EncoderDefaults {
	const request = bagRequest(bag);
	return {
		settings: request.encoderSettings,
		sampleRate: request.sampleRate,
		format: bag.format,
		intent: bag.intent,
	};
}

export function bagEstimateKbps(bag: EncodingBag): number | null {
	return estimateKbpsFromRequest(bagRequest(bag));
}

function availabilityHint(bag: EncodingBag): string {
	if (opus(bag)) return 'Opus · variable bitrate';
	if (!bag.availability) return DEFAULT_AVAILABILITY_HINT;
	const selected = bag.flavor;
	const effective = effectiveEncoder(bag);
	if (selected === 'auto') {
		if (effective === 'fdk_he_aac') return fdkAvailabilityHint(bag);
		if (effective === 'aac_at') return 'Auto will use Apple AAC. FDK AAC is not available.';
		return `Auto will use Native AAC (NMR). FDK AAC is not available. ${NATIVE_AAC_HINT}`;
	}
	if (effective === 'faac') return 'FAAC is included with ABB.';
	if (effective === 'native_aac') {
		if (!bag.availability.nativeAacAvailable) {
			return 'Native AAC (NMR) is unavailable in this build.';
		}
		return NATIVE_AAC_HINT;
	}
	if (effective === 'fdk_he_aac') {
		if (!bag.availability.fdkAvailable) {
			return 'FDK AAC needs an auto-detectable FFmpeg with libfdk_aac.';
		}
		return fdkAvailabilityHint(bag);
	}
	if (!bag.availability.aacAtAvailable) return 'Apple AAC is unavailable in this build.';
	return 'Apple AAC available';
}

function fdkAvailabilityHint(bag: EncodingBag): string {
	const path = compactToolchainPath(bag.availability?.detectedToolchainPath);
	const pathSegment = path ? ` via ${path}` : '';
	const afterburnerSegment = bag.afterburner ? 'Afterburner on.' : 'Afterburner off.';
	return `Using external FDK AAC${pathSegment}. ${afterburnerSegment}`;
}

function autoOptionLabel(bag: EncodingBag): string {
	if (bag.flavor === 'auto' && bag.availability) {
		return `App default (${encoderFlavorLabel(effectiveEncoder(bag))})`;
	}
	return 'App default';
}

function encoderLabel(bag: EncodingBag, value: string): string {
	if (value === 'auto') return autoOptionLabel(bag);
	if (value === 'fdk_he_aac' && bag.availability && !bag.availability.fdkAvailable)
		return 'FDK AAC (Set up…)';
	if (value === 'fdk_he_aac' || value === 'aac_at' || value === 'native_aac' || value === 'faac') {
		return encoderFlavorLabel(value);
	}
	return value === 'opus' ? 'Opus (libopus)' : value;
}

function qualityLabel(bag: EncodingBag, value: number): string {
	if (bag.flavor === 'faac') {
		const standard = bag.capabilities?.faacQualityDefault ?? 100;
		const label = value === standard ? 'Standard' : value < standard ? 'Smaller' : 'Higher';
		return `${label} (${value})`;
	}
	if (value === bag.capabilities?.vbrLevelMin) return `${value} (Smallest)`;
	if (value === bag.capabilities?.vbrLevelDefault) return `${value} (Recommended)`;
	if (value === bag.capabilities?.vbrLevelMax) return `${value} (Largest)`;
	return String(value);
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

function encoderConfiguration(bag: EncodingBag): EncoderConfigurationCapability | undefined {
	return bag.capabilities?.encoderConfigurations.find(
		(entry) => entry.encoderType === effectiveEncoder(bag),
	);
}

function sampleRateDetail(bag: EncodingBag): string {
	if (
		bag.capabilities &&
		bag.sampleRate !== 'auto' &&
		!allowedSampleRates(bag).includes(Number(bag.sampleRate))
	)
		return 'Choose a supported sample rate for this encoder.';
	if (bag.sampleRate === 'auto')
		return opus(bag) ? 'Auto → next supported Opus input rate' : bag.sampleRateHint;
	return `Using ${sampleRateLabel(bag.sampleRate)}.`;
}

function allowedSampleRates(bag: EncodingBag): readonly number[] {
	const configuration = encoderConfiguration(bag);
	if (!opus(bag) && bag.flavor === 'faac')
		return (
			configuration?.faacProfiles.find((entry) => entry.profile === bag.faacProfile)
				?.explicitSampleRates ?? []
		);
	return configuration?.explicitSampleRates ?? [];
}

function channelsDetail(bag: EncodingBag): string {
	if (bag.channels === 'auto') return bag.channelsHint;
	const downmix = bag.hasMultichannelInput ? ' Surround downmix omits bass effects (LFE).' : '';
	return `Using ${channelLabel(bag.channels)}.${downmix}`;
}

function autoResolutionLabel(hint: string): string {
	const detail = hint.replace(/^Auto\s*(?:->|→)\s*/, '');
	return detail === hint
		? 'Auto · Choose channels'
		: `Auto · ${detail.charAt(0).toUpperCase()}${detail.slice(1)}`;
}

export function projectView(bag: EncodingBag): EncodingView {
	const effective = effectiveEncoder(bag);
	const estimate = bagEstimateKbps(bag);
	const showQuality = bitrateModeFromBag(bag).mode === 'vbr';
	const native = effectiveEncoder(bag) === 'native_aac';
	const faac = !opus(bag) && bag.flavor === 'faac';
	const disabled = disabledEncoderOptions(bag.availability);
	const flavorOptions: EncodingOption[] =
		bag.capabilities === null
			? [{ value: 'auto', label: 'Loading…', disabled: true }]
			: bag.capabilities.encoderTypes
					.filter((flavor) => (opus(bag) ? flavor === 'opus' : flavor !== 'opus'))
					.map((flavor) => ({
						value: flavor,
						label: encoderLabel(bag, flavor),
						disabled:
							flavor !== 'auto' &&
							flavor !== 'fdk_he_aac' &&
							Boolean(disabled[flavor as keyof typeof disabled]),
					}));
	const qualityOptions = bag.capabilities
		? (faac
				? bag.capabilities.faacQualityPresets
				: rangeOptions(bag.capabilities.vbrLevelMin, bag.capabilities.vbrLevelMax)
			).map((value) => ({
				value: String(value),
				label: qualityLabel(bag, value),
			}))
		: [];
	const sampleRateOptions = bag.capabilities
		? [
				...(bag.capabilities.sampleRateAuto ? ['auto'] : []),
				...bag.capabilities.explicitSampleRates.map(String),
			].map((value) => ({
				value,
				label:
					value === 'auto'
						? autoResolutionLabel(sampleRateDetail({ ...bag, sampleRate: 'auto' }))
						: sampleRateLabel(value),
				disabled: value !== 'auto' && !allowedSampleRates(bag).includes(Number(value)),
			}))
		: [];
	const channelOptions = (bag.capabilities?.channelOptions ?? []).map((value) => ({
		value,
		label: value === 'auto' ? autoResolutionLabel(bag.channelsHint) : channelLabel(value),
	}));

	return {
		format: bag.format,
		intent: bag.intent,
		flavor: opus(bag) ? 'opus' : bag.flavor,
		flavorOptions,
		flavorDisabled: opus(bag) || bag.capabilities === null,
		availabilityHint: availabilityHint(bag),
		fdkSetupNeeded: !opus(bag) && bag.availability !== null && !bag.availability.fdkAvailable,
		profileDisplay:
			effective === 'faac' ? FAAC_PROFILES[bag.faacProfile] : ENCODER_PROFILES[effective],
		qualityBitrateLabel: showQuality ? 'Quality' : 'Bitrate (kbps)',
		native,
		faac,
		faacProfile: bag.faacProfile,
		faacProfileOptions: (encoderConfiguration(bag)?.faacProfiles ?? []).map(({ profile }) => ({
			value: profile,
			label: FAAC_PROFILES[profile],
		})),
		rateControl: bag.faacMode,
		rateControlOptions: (encoderConfiguration(bag)?.allowedModes ?? []).map((mode) => ({
			value: mode,
			label: mode === 'abr' ? 'Average bitrate (ABR)' : 'Quality (VBR)',
		})),
		profileHint:
			bag.faacProfile === 'auto' ? 'FAAC chooses LC or HE once from the output settings.' : '',

		bitrateKbpsMax: encoderConfiguration(bag)?.bitrateKbpsMax ?? 0,
		nativeSpeed: bag.nativeSpeed,
		nativeSpeedOptions: rangeOptions(0, bag.capabilities?.nativeSpeedMax ?? 0).map((value) => ({
			value: String(value),
			label:
				value === 0
					? '0 · Full search (default)'
					: value === 4
						? '4 · Fastest search'
						: String(value),
		})),
		showQuality,
		quality: faac ? bag.faacQuality : bag.quality,
		qualityOptions,
		bitrate: opus(bag) ? bag.opusBitrate : bag.bitrate,
		bitrateKbpsMin: encoderConfiguration(bag)?.bitrateKbpsMin ?? 1,
		estimatedBitrateText:
			faac && showQuality
				? 'Size varies. Start with Standard, or choose ABR for a bitrate target.'
				: showQuality
					? `Est: ~${estimate} kbps`
					: '',
		sampleRate: bag.sampleRate,
		sampleRateOptions,
		sampleRateDisabled: sampleRateOptions.length === 0,
		sampleRateHint: sampleRateDetail(bag),
		channels: bag.channels,
		channelOptions,
		channelsDisabled: channelOptions.length === 0,
		channelsHint: channelsDetail(bag),
		afterburner: bag.afterburner,
	};
}

export type SyncResult = {
	readonly flavorReset: boolean;
};

export function syncPolicy(bag: EncodingBag): SyncResult {
	const flavorReset = flavorIsUnavailable(bag);
	if (flavorReset) {
		bag.flavor = 'auto';
	}
	return { flavorReset };
}

export function applyCapabilities(
	bag: EncodingBag,
	capabilities: EncoderSettingsCapabilities | null,
): void {
	bag.capabilities = capabilities;
	bag.availability = capabilities?.availability ?? null;
	if (!capabilities) return;
	for (const [format, field] of [
		['m4b', 'bitrate'],
		['m4aOpus', 'opusBitrate'],
	] as const) {
		const configuration = encoderConfiguration({ ...bag, format });
		if (configuration)
			bag[field] = Math.min(
				configuration.bitrateKbpsMax,
				Math.max(configuration.bitrateKbpsMin, bag[field]),
			);
	}
	bag.quality = Math.min(capabilities.vbrLevelMax, Math.max(capabilities.vbrLevelMin, bag.quality));
	if (!capabilities.faacQualityPresets.includes(bag.faacQuality))
		bag.faacQuality = capabilities.faacQualityDefault;
	bag.nativeSpeed = Math.min(capabilities.nativeSpeedMax, Math.max(0, bag.nativeSpeed));
	const sampleRates = [
		...(capabilities.sampleRateAuto ? ['auto'] : []),
		...capabilities.explicitSampleRates.map(String),
	];
	if (!sampleRates.includes(bag.sampleRate)) {
		bag.sampleRate = capabilities.sampleRateAuto ? 'auto' : (sampleRates[0] ?? 'auto');
	}
	if (!capabilities.channelOptions.includes(bag.channels)) {
		bag.channels = capabilities.channelOptions[0] ?? bag.channels;
	}
}

export function applyDefaultsToBag(bag: EncodingBag, defaults: EncoderDefaults): void {
	const settings = defaults.settings;
	bag.flavor = settings.encoderType === 'opus' ? bag.flavor : settings.encoderType;
	bag.format = defaults.format ?? 'm4b';
	bag.intent = defaults.intent ?? 'auto';
	if (settings.encoderType === 'opus') bag.opusBitrate = settings.bitrateKbps;
	bag.hydratedBitrateMode = settings.bitrateMode;
	bag.faacProfile = settings.faacProfile ?? 'auto';
	if (settings.encoderType === 'faac') {
		bag.faacMode = settings.bitrateMode.mode === 'vbr' ? 'vbr' : 'abr';
		if (settings.bitrateMode.mode === 'vbr') bag.faacQuality = settings.bitrateMode.value;
	} else if (settings.bitrateMode.mode === 'vbr') bag.quality = settings.bitrateMode.value;
	bag.nativeSpeed = settings.nativeAacSpeed ?? 0;
	bag.bitrate = settings.bitrateKbps;
	bag.channels = settings.channels;
	bag.afterburner = settings.afterburner;
	bag.sampleRate = defaults.sampleRate === 'auto' ? 'auto' : String(defaults.sampleRate.explicit);
	applyCapabilities(bag, bag.capabilities);
}

export function selectField(bag: EncodingBag, field: EncodingField, value: string): boolean {
	switch (field) {
		case 'format':
			if (!['m4b', 'mp3', 'm4aOpus', 'mkaOpus'].includes(value)) return false;
			bag.format = value as AudiobookFormat;
			if (value === 'mp3') bag.intent = 'preserve';
			return true;
		case 'intent':
			if (value !== 'auto' && value !== 'encode' && value !== 'preserve') return false;
			bag.intent = value;
			return true;
		case 'encoder': {
			if (
				value !== 'auto' &&
				value !== 'fdk_he_aac' &&
				value !== 'aac_at' &&
				value !== 'native_aac' &&
				value !== 'faac'
			) {
				return false;
			}
			if (bag.flavor === value) return false;
			bag.flavor = value;
			return true;
		}
		case 'nativeSpeed': {
			const speed = Number(value);
			if (
				!bag.capabilities ||
				!Number.isInteger(speed) ||
				speed < 0 ||
				speed > bag.capabilities.nativeSpeedMax ||
				speed === bag.nativeSpeed
			)
				return false;
			bag.nativeSpeed = speed;
			return true;
		}
		case 'faacProfile': {
			if (value !== 'auto' && value !== 'aac_lc' && value !== 'he_aac_v1') return false;
			if (bag.faacProfile === value) return false;
			bag.faacProfile = value;
			return true;
		}
		case 'rateControl': {
			if (bag.flavor !== 'faac' || (value !== 'abr' && value !== 'vbr') || bag.faacMode === value)
				return false;
			bag.faacMode = value;
			return true;
		}
		case 'quality': {
			if (bag.flavor === 'faac') {
				const quality = Number(value);
				if (!bag.capabilities?.faacQualityPresets.includes(quality) || bag.faacQuality === quality)
					return false;
				bag.faacQuality = quality;
				return true;
			}
			const parsed = Number.parseInt(value, 10);
			if (!Number.isFinite(parsed)) return false;
			const min = bag.capabilities?.vbrLevelMin ?? parsed;
			const max = bag.capabilities?.vbrLevelMax ?? parsed;
			const next = Math.min(max, Math.max(min, parsed));
			if (bag.quality === next) return false;
			bag.quality = next;
			return true;
		}
		case 'bitrate': {
			const parsed = Number(value);
			const configuration = encoderConfiguration(bag);
			if (
				!Number.isInteger(parsed) ||
				!value.trim() ||
				!configuration ||
				parsed < configuration.bitrateKbpsMin ||
				parsed > configuration.bitrateKbpsMax
			)
				return false;
			const field = opus(bag) ? 'opusBitrate' : 'bitrate';
			if (bag[field] === parsed) return false;
			bag[field] = parsed;
			return true;
		}
		case 'sampleRate': {
			const options = bag.capabilities
				? [
						...(bag.capabilities.sampleRateAuto ? ['auto'] : []),
						...bag.capabilities.explicitSampleRates.map(String),
					]
				: ['auto'];
			if (value !== 'auto' && !allowedSampleRates(bag).includes(Number(value))) return false;
			const next = options.includes(value) ? value : 'auto';
			if (bag.sampleRate === next) return false;
			bag.sampleRate = next;
			return true;
		}
		case 'channels': {
			if (
				value !== 'auto' &&
				value !== 'mono' &&
				value !== 'stereo' &&
				!(bag.capabilities?.channelOptions.includes(value as EncodingBag['channels']) ?? false)
			) {
				return false;
			}
			if (
				bag.capabilities &&
				!bag.capabilities.channelOptions.includes(value as EncodingBag['channels'])
			) {
				return false;
			}
			if (bag.channels === value) return false;
			bag.channels = value as EncodingBag['channels'];
			return true;
		}
	}
}
