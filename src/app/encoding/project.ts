import type { EncoderDefaults } from '../../types/appSettings';
import type {
	BitrateMode,
	EncoderAvailability,
	EncoderChannelConfig,
	EncoderSettingsCapabilities,
	EncoderType,
	EncodingRequestConfig,
	SampleRateConfig,
} from '../../types/audio';
import { defaultEncoderSettings } from '../../types/audio';
import { estimateKbpsFromRequest } from './estimate';

export type EncodingField =
	| 'encoder'
	| 'quality'
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
	readonly flavor: string;
	readonly flavorOptions: ReadonlyArray<EncodingOption>;
	readonly flavorDisabled: boolean;
	readonly availabilityHint: string;
	readonly fdkSetupNeeded: boolean;
	readonly profileDisplay: string;
	readonly qualityBitrateLabel: 'Quality' | 'Target kbps';
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
	flavor: EncoderType;
	hydratedBitrateMode: BitrateMode;
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
const ENCODER_PROFILES: Record<EncoderType, string> = {
	auto: 'HE-AAC v1',
	fdk_he_aac: 'HE-AAC v1',
	aac_at: 'AAC-LC',
	native_aac: 'AAC-LC',
};

export function createDefaultBag(): EncodingBag {
	return {
		flavor: 'auto',
		hydratedBitrateMode: defaultEncoderSettings().bitrateMode,
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
		default:
			return 'Auto';
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

function effectiveEncoder(bag: EncodingBag): EncoderType {
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
	const mode =
		bag.capabilities?.bitrateModesByEncoder.find(
			(entry) => entry.encoderType === effectiveEncoder(bag),
		)?.defaultMode ?? bag.hydratedBitrateMode;
	return mode.mode === 'vbr' ? { mode: 'vbr', value: bag.quality } : mode;
}

export function bagRequest(bag: EncodingBag): EncodingRequestConfig {
	return {
		encoderSettings: {
			encoderType: bag.flavor,
			channels: bag.channels,
			bitrateKbps: bag.bitrate,
			bitrateMode: bitrateModeFromBag(bag),
			afterburner: bag.afterburner,
			nativeAacSpeed: bag.nativeSpeed,
		},
		sampleRate: sampleRateFromBag(bag),
	};
}

export function bagDefaults(bag: EncodingBag): EncoderDefaults {
	const request = bagRequest(bag);
	return {
		settings: request.encoderSettings,
		sampleRate: request.sampleRate,
	};
}

export function bagEstimateKbps(bag: EncodingBag): number {
	return estimateKbpsFromRequest(bagRequest(bag));
}

function availabilityHint(bag: EncodingBag): string {
	if (!bag.availability) return DEFAULT_AVAILABILITY_HINT;
	const selected = bag.flavor;
	const effective = effectiveEncoder(bag);
	if (selected === 'auto') {
		if (effective === 'fdk_he_aac') return fdkAvailabilityHint(bag);
		if (effective === 'aac_at') return 'Auto will use Apple AAC. FDK AAC is not available.';
		return `Auto will use Native AAC (NMR). FDK AAC is not available. ${NATIVE_AAC_HINT}`;
	}
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
		return `Auto (${encoderFlavorLabel(effectiveEncoder(bag))})`;
	}
	return 'Auto';
}

function encoderLabel(bag: EncodingBag, value: string): string {
	if (value === 'auto') return autoOptionLabel(bag);
	if (value === 'fdk_he_aac' && bag.availability && !bag.availability.fdkAvailable)
		return 'FDK AAC (Set up…)';
	if (value === 'fdk_he_aac' || value === 'aac_at' || value === 'native_aac') {
		return encoderFlavorLabel(value);
	}
	return value;
}

function qualityLabel(bag: EncodingBag, value: number): string {
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

function sampleRateDetail(bag: EncodingBag): string {
	if (bag.sampleRate === 'auto') return bag.sampleRateHint;
	return `Using ${sampleRateLabel(bag.sampleRate)}.`;
}

function channelsDetail(bag: EncodingBag): string {
	if (bag.channels === 'auto') return bag.channelsHint;
	const downmix = bag.hasMultichannelInput ? ' Surround downmix omits bass effects (LFE).' : '';
	return `Using ${channelLabel(bag.channels)}.${downmix}`;
}

export function projectView(bag: EncodingBag): EncodingView {
	const estimate = bagEstimateKbps(bag);
	const showQuality = bitrateModeFromBag(bag).mode === 'vbr';
	const native = effectiveEncoder(bag) === 'native_aac';
	const disabled = disabledEncoderOptions(bag.availability);
	const flavorOptions: EncodingOption[] =
		bag.capabilities === null
			? [{ value: 'auto', label: 'Loading…', disabled: true }]
			: bag.capabilities.encoderTypes.map((flavor) => ({
					value: flavor,
					label: encoderLabel(bag, flavor),
					disabled:
						flavor !== 'auto' &&
						flavor !== 'fdk_he_aac' &&
						Boolean(disabled[flavor as keyof typeof disabled]),
				}));
	const qualityOptions = bag.capabilities
		? rangeOptions(bag.capabilities.vbrLevelMin, bag.capabilities.vbrLevelMax).map((value) => ({
				value: String(value),
				label: qualityLabel(bag, value),
			}))
		: [];
	const sampleRateOptions = bag.capabilities
		? [
				...(bag.capabilities.sampleRateAuto ? ['auto'] : []),
				...bag.capabilities.explicitSampleRates.map(String),
			].map((value) => ({ value, label: sampleRateLabel(value) }))
		: [];
	const channelOptions = (bag.capabilities?.channelOptions ?? []).map((value) => ({
		value,
		label: channelLabel(value),
	}));

	return {
		flavor: bag.flavor,
		flavorOptions,
		flavorDisabled: bag.capabilities === null,
		availabilityHint: availabilityHint(bag),
		fdkSetupNeeded: bag.availability !== null && !bag.availability.fdkAvailable,
		profileDisplay: ENCODER_PROFILES[effectiveEncoder(bag)] ?? 'AAC-LC',
		qualityBitrateLabel: showQuality ? 'Quality' : 'Target kbps',
		native,
		bitrateKbpsMax: bag.capabilities?.bitrateKbpsMax ?? 0,
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
		quality: bag.quality,
		qualityOptions,
		bitrate: bag.bitrate,
		bitrateKbpsMin: bag.capabilities?.bitrateKbpsMin ?? 1,
		estimatedBitrateText: showQuality ? `Est: ~${estimate} kbps` : `Target: ${estimate} kbps total`,
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
	bag.bitrate = Math.min(
		capabilities.bitrateKbpsMax,
		Math.max(capabilities.bitrateKbpsMin, bag.bitrate),
	);
	bag.quality = Math.min(capabilities.vbrLevelMax, Math.max(capabilities.vbrLevelMin, bag.quality));
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
	bag.flavor = settings.encoderType;
	bag.hydratedBitrateMode = settings.bitrateMode;
	if (settings.bitrateMode.mode === 'vbr') bag.quality = settings.bitrateMode.value;
	bag.nativeSpeed = settings.nativeAacSpeed ?? 0;
	bag.bitrate = settings.bitrateKbps;
	bag.channels = settings.channels;
	bag.afterburner = settings.afterburner;
	bag.sampleRate = defaults.sampleRate === 'auto' ? 'auto' : String(defaults.sampleRate.explicit);
	applyCapabilities(bag, bag.capabilities);
}

export function selectField(bag: EncodingBag, field: EncodingField, value: string): boolean {
	switch (field) {
		case 'encoder': {
			if (
				value !== 'auto' &&
				value !== 'fdk_he_aac' &&
				value !== 'aac_at' &&
				value !== 'native_aac'
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
		case 'quality': {
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
			if (!Number.isInteger(parsed) || !value.trim()) return false;
			if (
				!bag.capabilities ||
				parsed < bag.capabilities.bitrateKbpsMin ||
				parsed > bag.capabilities.bitrateKbpsMax
			) {
				return false;
			}
			if (bag.bitrate === parsed) return false;
			bag.bitrate = parsed;
			return true;
		}
		case 'sampleRate': {
			const options = bag.capabilities
				? [
						...(bag.capabilities.sampleRateAuto ? ['auto'] : []),
						...bag.capabilities.explicitSampleRates.map(String),
					]
				: ['auto'];
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
