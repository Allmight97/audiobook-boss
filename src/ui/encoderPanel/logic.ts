import { updateEstimatedSize } from '../outputPanel';
import {
	applyEncoderDefaults,
	encoderPanelState,
	readEncoderDefaultsFromState,
	setEncoderSettingsCapabilities,
	type BitrateModeSelection,
	type VbrLevel,
} from './state.svelte';
import type { EncoderFlavor } from '../../types/encoder';
import type { EncoderAvailability, EncoderSettingsCapabilities } from '../../types/audio';
import type { EncoderDefaults } from '../../types/appSettings';
import { resetAutoResolutionHints } from './autoResolutionHints';
import { persistEncoderDefaults } from '../appSettings/persistence';
import { loadRuntimeSettingsCapabilities } from '../runtimeSettingsCapabilities.svelte';

const DEBUG = import.meta.env.DEV;
const debugLog = (...args: unknown[]): void => {
	if (DEBUG) console.log('[EncoderPanel]', ...args);
};

const ENCODER_PROFILES: Record<EncoderFlavor, string> = {
	auto: 'HE-AAC v1',
	fdk_he_aac: 'HE-AAC v1',
	aac_at: 'AAC-LC',
	native_aac: 'AAC-LC',
};
const VBR_BITRATE_ESTIMATES: Record<number, number> = {
	1: 32,
	2: 48,
	3: 60,
	4: 72,
	5: 96,
};
const NATIVE_AAC_WARNING =
	'Native AAC (FFmpeg) may sound degraded on speech (known issue; prefer Auto/Apple/FDK).';
const AUTO_LABEL_BASE = 'Auto';

const clamp = (value: number, min: number, max: number): number =>
	Math.min(max, Math.max(min, value));

const selectTarget = (event: Event): HTMLSelectElement | null =>
	(event.currentTarget ?? event.target) as HTMLSelectElement | null;

const inputTarget = (event: Event): HTMLInputElement | null =>
	(event.currentTarget ?? event.target) as HTMLInputElement | null;

const resolveEffectiveEncoder = (flavor: EncoderFlavor): EncoderFlavor => {
	if (flavor !== 'auto') return flavor;
	return encoderPanelState.availability?.autoEncoder ?? 'auto';
};

const encoderFlavorLabel = (flavor: EncoderFlavor): string => {
	switch (flavor) {
		case 'fdk_he_aac':
			return 'FDK AAC';
		case 'aac_at':
			return 'Apple AAC';
		case 'native_aac':
			return 'Native AAC (FFmpeg)';
		default:
			return AUTO_LABEL_BASE;
	}
};

const autoOptionLabel = (effectiveEncoder: EncoderFlavor): string =>
	`${AUTO_LABEL_BASE} (${encoderFlavorLabel(effectiveEncoder)})`;

const compactToolchainPath = (path: string | null | undefined): string | null => {
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
};

const fdkAvailabilityHint = (): string => {
	const path = compactToolchainPath(encoderPanelState.availability?.detectedToolchainPath);
	const pathSegment = path ? ` via ${path}` : '';
	const afterburnerSegment = encoderPanelState.fdkAfterburner
		? 'Afterburner on.'
		: 'Afterburner off.';
	return `Using external FDK AAC${pathSegment}. ${afterburnerSegment}`;
};

const syncOutputSizingFromEncoderState = (): void => {
	updateEstimatedSize();
};

const updateAutoOptionLabel = (): void => {
	if (encoderPanelState.flavor === 'auto' && encoderPanelState.availability) {
		encoderPanelState.autoOptionLabel = autoOptionLabel(resolveEffectiveEncoder('auto'));
		return;
	}

	encoderPanelState.autoOptionLabel = AUTO_LABEL_BASE;
};

const updateAvailabilityHint = (): void => {
	if (!encoderPanelState.availability) {
		encoderPanelState.availabilityHint = 'Checking encoder availability…';
		return;
	}

	const selectedFlavor = encoderPanelState.flavor;
	const effectiveEncoder = resolveEffectiveEncoder(selectedFlavor);

	if (selectedFlavor === 'auto') {
		if (effectiveEncoder === 'fdk_he_aac') {
			encoderPanelState.availabilityHint = fdkAvailabilityHint();
			return;
		}
		if (effectiveEncoder === 'aac_at') {
			encoderPanelState.availabilityHint = 'Auto will use Apple AAC.';
			return;
		}
		encoderPanelState.availabilityHint = `Auto will use Native AAC (FFmpeg). ${NATIVE_AAC_WARNING}`;
		return;
	}

	if (effectiveEncoder === 'native_aac') {
		if (!encoderPanelState.availability.nativeAacAvailable) {
			encoderPanelState.availabilityHint = 'Native AAC (FFmpeg) is unavailable in this build.';
			return;
		}
		encoderPanelState.availabilityHint = NATIVE_AAC_WARNING;
		return;
	}

	if (effectiveEncoder === 'fdk_he_aac') {
		if (!encoderPanelState.availability.fdkAvailable) {
			encoderPanelState.availabilityHint =
				'FDK AAC needs an auto-detectable FFmpeg with libfdk_aac.';
			return;
		}
		encoderPanelState.availabilityHint = fdkAvailabilityHint();
		return;
	}

	if (!encoderPanelState.availability.aacAtAvailable) {
		encoderPanelState.availabilityHint = 'Apple AAC is unavailable in this build.';
		return;
	}

	encoderPanelState.availabilityHint = 'Apple AAC available';
};

const updateProfileDisplay = (encoder: EncoderFlavor): void => {
	encoderPanelState.profileDisplay = ENCODER_PROFILES[encoder] ?? 'AAC-LC';
};

function bitrateModeSelectionFromKind(kind: string): BitrateModeSelection {
	return kind === 'cvbr' || kind === 'cbr' ? kind : 'vbr';
}

function bitrateModeSelectionFromMode(mode: { mode: string }): BitrateModeSelection {
	return bitrateModeSelectionFromKind(mode.mode);
}

function capabilityForEncoder(
	encoder: EncoderFlavor,
	capabilities: EncoderSettingsCapabilities | null,
) {
	return capabilities?.bitrateModesByEncoder.find((entry) => entry.encoderType === encoder) ?? null;
}

const enforceBitrateModeCompatibility = (encoder: EncoderFlavor): void => {
	const capability = capabilityForEncoder(encoder, encoderPanelState.capabilities);
	const allowedModes = capability?.allowedModes.map(bitrateModeSelectionFromKind) ?? [];
	const defaultMode = capability
		? bitrateModeSelectionFromMode(capability.defaultMode)
		: encoderPanelState.bitrateModeSelection;
	encoderPanelState.bitrateModeAvailability = {
		vbr: allowedModes.includes('vbr'),
		cvbr: allowedModes.includes('cvbr'),
		cbr: allowedModes.includes('cbr'),
	};
	if (!allowedModes.includes(encoderPanelState.bitrateModeSelection)) {
		encoderPanelState.bitrateModeSelection = defaultMode;
	}
};

const updateQualityVisibility = (): void => {
	encoderPanelState.showQuality = encoderPanelState.bitrateModeSelection === 'vbr';
	encoderPanelState.qualityBitrateLabel = encoderPanelState.showQuality ? 'Quality' : 'Bitrate';
};

const updateInlineOptions = (effectiveEncoder: EncoderFlavor): void => {
	encoderPanelState.showFdkOptions = effectiveEncoder === 'fdk_he_aac';
	encoderPanelState.showNativeOptions = effectiveEncoder === 'native_aac';
	encoderPanelState.showInlineOptionRow =
		encoderPanelState.showFdkOptions || encoderPanelState.showNativeOptions;
};

const updateEstimatedBitrate = (): void => {
	if (encoderPanelState.bitrateModeSelection === 'vbr') {
		const estimate =
			VBR_BITRATE_ESTIMATES[encoderPanelState.qualityValue] ??
			VBR_BITRATE_ESTIMATES[encoderPanelState.capabilities?.vbrLevelDefault ?? 3] ??
			encoderPanelState.bitrateValue;
		encoderPanelState.estimatedBitrateText = `Est: ~${estimate} kbps`;
		return;
	}

	encoderPanelState.estimatedBitrateText = `Target: ${encoderPanelState.bitrateValue} kbps`;
};

const getDisabledEncoderOptions = (
	availability: EncoderAvailability | null,
): typeof encoderPanelState.disabledEncoderOptions => ({
	fdk_he_aac: !availability?.fdkAvailable,
	aac_at: availability ? !availability.aacAtAvailable : false,
	native_aac: availability ? !availability.nativeAacAvailable : false,
});

const enforceAllowedEncoderSelection = (): void => {
	const disabledOptions = getDisabledEncoderOptions(encoderPanelState.availability);
	encoderPanelState.disabledEncoderOptions = disabledOptions;
};

const normalizeUnavailableFlavorSelection = (): boolean => {
	const availability = encoderPanelState.availability;
	if (!availability || encoderPanelState.flavor === 'auto') {
		return false;
	}

	const disabledOptions = getDisabledEncoderOptions(availability);
	const shouldReset =
		(encoderPanelState.flavor === 'fdk_he_aac' && disabledOptions.fdk_he_aac) ||
		(encoderPanelState.flavor === 'aac_at' && disabledOptions.aac_at) ||
		(encoderPanelState.flavor === 'native_aac' && disabledOptions.native_aac);

	if (!shouldReset) {
		return false;
	}

	encoderPanelState.flavor = 'auto';
	return true;
};

const syncEncoderState = (): void => {
	normalizeUnavailableFlavorSelection();
	enforceAllowedEncoderSelection();
	const effectiveEncoder = resolveEffectiveEncoder(encoderPanelState.flavor);
	enforceBitrateModeCompatibility(effectiveEncoder);
	updateQualityVisibility();
	updateInlineOptions(effectiveEncoder);
	updateProfileDisplay(effectiveEncoder);
	updateEstimatedBitrate();
	updateAutoOptionLabel();
	updateAvailabilityHint();
};

export const syncAfterStateChange = (): void => {
	syncEncoderState();
	syncOutputSizingFromEncoderState();
};

const persistCurrentEncoderDefaults = (): void => {
	void persistEncoderDefaults(readEncoderDefaultsFromState());
};

export const syncEncoderPanelAfterAvailabilityChange = (): void => {
	syncEncoderState();
	syncOutputSizingFromEncoderState();
	debugLog('Encoder panel ready');
};

export const setRuntimeEncoderSettingsCapabilities = (
	capabilities: EncoderSettingsCapabilities | null,
): void => {
	setEncoderSettingsCapabilities(capabilities);
	syncEncoderPanelAfterAvailabilityChange();
};

const hydrateEncoderAvailability = async (): Promise<void> => {
	const capabilities = await loadRuntimeSettingsCapabilities();
	setRuntimeEncoderSettingsCapabilities(capabilities?.encoder ?? null);
};

export const initializeEncoderPanelLogic = (): void => {
	debugLog('Initializing encoder panel...');
	resetAutoResolutionHints();
	syncOutputSizingFromEncoderState();
	void hydrateEncoderAvailability();
};

export const applyEncodingDefaults = (defaults: EncoderDefaults): void => {
	applyEncoderDefaults(defaults);
	syncAfterStateChange();
};

export const handleFlavorChange = (event: Event): void => {
	const target = selectTarget(event);
	encoderPanelState.flavor = (target?.value as EncoderFlavor | undefined) ?? 'auto';
	syncAfterStateChange();
	persistCurrentEncoderDefaults();
};

export const handleBitrateModeChange = (event: Event): void => {
	const target = selectTarget(event);
	const value = target?.value;
	if (value === 'vbr' || value === 'cvbr' || value === 'cbr') {
		encoderPanelState.bitrateModeSelection = value;
	}
	syncAfterStateChange();
	persistCurrentEncoderDefaults();
};

export const handleChannelsSelectionChange = (event: Event): void => {
	const target = selectTarget(event);
	const value = target?.value;
	if (
		value &&
		encoderPanelState.channelOptions.includes(
			value as NonNullable<typeof encoderPanelState.channelsSelection>,
		)
	) {
		encoderPanelState.channelsSelection = value as typeof encoderPanelState.channelsSelection;
	}
	syncAfterStateChange();
	persistCurrentEncoderDefaults();
};

export const handleQualityValueChange = (event: Event): void => {
	const target = selectTarget(event);
	const value = Number.parseInt(
		target?.value ?? String(encoderPanelState.capabilities?.vbrLevelDefault ?? 3),
		10,
	);
	const min = encoderPanelState.capabilities?.vbrLevelMin ?? value;
	const max = encoderPanelState.capabilities?.vbrLevelMax ?? value;
	encoderPanelState.qualityValue = clamp(value, min, max) as VbrLevel;
	syncAfterStateChange();
	persistCurrentEncoderDefaults();
};

export const handleBitrateValueChange = (event: Event): void => {
	const target = selectTarget(event);
	const value = Number.parseInt(target?.value ?? '64', 10);
	if (
		Number.isFinite(value) &&
		encoderPanelState.capabilities?.bitrateKbpsOptions.includes(value)
	) {
		encoderPanelState.bitrateValue = value as typeof encoderPanelState.bitrateValue;
	}
	syncAfterStateChange();
	persistCurrentEncoderDefaults();
};

export const handleFdkAfterburnerChange = (event: Event): void => {
	const target = inputTarget(event);
	encoderPanelState.fdkAfterburner = Boolean(target?.checked);
	syncAfterStateChange();
	persistCurrentEncoderDefaults();
};

export const handleNativeTwoloopChange = (event: Event): void => {
	const target = inputTarget(event);
	encoderPanelState.nativeTwoloop = Boolean(target?.checked);
	syncAfterStateChange();
	persistCurrentEncoderDefaults();
};

export const handleSampleRateSelectionChange = (event: Event): void => {
	const target = selectTarget(event);
	const value = target?.value ?? 'auto';
	encoderPanelState.sampleRateSelection = encoderPanelState.sampleRateOptions.includes(value)
		? value
		: 'auto';
	syncAfterStateChange();
	persistCurrentEncoderDefaults();
};
