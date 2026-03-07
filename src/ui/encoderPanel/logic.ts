import { ENABLE_FDK } from './featureFlags';
import { loadState, saveState } from './state';
import { updateEstimatedSize } from '../outputPanel/dom';
import { updateEncoderSettings, updateSampleRate } from '../outputPanel/state';
import {
	applyPersistedEncoderState,
	encoderPanelState,
	readBoundaryEncoderSettings,
	readPersistedEncoderState,
	readSampleRateFromState,
	setEncoderAvailability,
	type EncoderAvailability,
	type BitrateModeSelection,
	type VbrLevel,
} from './state.svelte';
import type { EncoderFlavor } from '../../types/encoder';
import { tauriClient } from '../../lib/tauri/client';
import { resetAutoResolutionHints } from './autoResolutionHints';

/** Debug logging - only active in development builds */
const DEBUG = import.meta.env.DEV;
const debugLog = (...args: unknown[]): void => {
	if (DEBUG) console.log('[EncoderPanel]', ...args);
};

const PERSIST_DEBOUNCE_MS = 300;
const ENCODER_PROFILES: Record<EncoderFlavor, string> = {
	auto: 'HE-AAC v1',
	fdk_he_aac: 'HE-AAC v1',
	aac_at: 'AAC-LC',
	native_aac: 'AAC-LC',
};
const VBR_BITRATE_ESTIMATES: Record<VbrLevel, number> = {
	1: 32,
	2: 48,
	3: 60,
	4: 72,
	5: 96,
};
const NATIVE_AAC_WARNING =
	'Native AAC (FFmpeg) may sound degraded on speech (known issue; prefer Auto/Apple/FDK).';
const AUTO_LABEL_BASE = 'Auto';

let persistTimer: ReturnType<typeof setTimeout> | null = null;
let fallbackHintTimer: ReturnType<typeof setTimeout> | null = null;

const clamp = (value: number, min: number, max: number): number =>
	Math.min(max, Math.max(min, value));

const resolveEffectiveEncoder = (flavor: EncoderFlavor): EncoderFlavor => {
	if (flavor !== 'auto') return flavor;
	if (encoderPanelState.availability?.fdkAvailable && ENABLE_FDK) return 'fdk_he_aac';
	if (encoderPanelState.availability?.aacAtAvailable) return 'aac_at';
	return 'native_aac';
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

const syncOutputState = (): void => {
	const settings = readBoundaryEncoderSettings();
	const sampleRate = readSampleRateFromState();
	updateEncoderSettings(settings);
	updateSampleRate(sampleRate);
};

const syncOutputSizingFromEncoderState = (): void => {
	syncOutputState();
	updateEstimatedSize();
};

const queuePersistState = (): void => {
	if (persistTimer) {
		clearTimeout(persistTimer);
	}
	persistTimer = setTimeout(() => {
		saveState(readPersistedEncoderState());
		persistTimer = null;
	}, PERSIST_DEBOUNCE_MS);
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
			encoderPanelState.availabilityHint = 'Auto will use FDK AAC.';
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
		encoderPanelState.availabilityHint = NATIVE_AAC_WARNING;
		return;
	}

	if (effectiveEncoder === 'fdk_he_aac') {
		encoderPanelState.availabilityHint = 'FDK detected ✓';
		return;
	}

	encoderPanelState.availabilityHint = 'Apple AAC available';
};

const updateProfileDisplay = (encoder: EncoderFlavor): void => {
	encoderPanelState.profileDisplay = ENCODER_PROFILES[encoder] ?? 'AAC-LC';
};

const enforceBitrateModeCompatibility = (encoder: EncoderFlavor): void => {
	const allowedModes: Record<EncoderFlavor, BitrateModeSelection> = {
		auto: 'vbr',
		fdk_he_aac: 'vbr',
		aac_at: 'cvbr',
		native_aac: 'cbr',
	};
	const allowedMode = allowedModes[encoder];
	encoderPanelState.bitrateModeAvailability = {
		vbr: allowedMode === 'vbr',
		cvbr: allowedMode === 'cvbr',
		cbr: allowedMode === 'cbr',
	};
	encoderPanelState.bitrateModeSelection = allowedMode;
};

const updateQualityVisibility = (): void => {
	encoderPanelState.showQuality = encoderPanelState.bitrateModeSelection === 'vbr';
	encoderPanelState.qualityBitrateLabel = encoderPanelState.showQuality ? 'Quality' : 'Bitrate';
};

const updateInlineOptions = (): void => {
	encoderPanelState.showInlineOptionRow =
		encoderPanelState.flavor === 'fdk_he_aac' || encoderPanelState.flavor === 'native_aac';
	encoderPanelState.showFdkOptions = encoderPanelState.flavor === 'fdk_he_aac';
	encoderPanelState.showNativeOptions = encoderPanelState.flavor === 'native_aac';
};

const updateEstimatedBitrate = (): void => {
	if (encoderPanelState.bitrateModeSelection === 'vbr') {
		const estimate = VBR_BITRATE_ESTIMATES[encoderPanelState.qualityValue];
		encoderPanelState.estimatedBitrateText = `Est: ~${estimate} kbps`;
		return;
	}

	encoderPanelState.estimatedBitrateText = `Target: ${encoderPanelState.bitrateValue} kbps`;
};

const reportEncoderAutoSwitchFallback = (from: EncoderFlavor, to: EncoderFlavor): void => {
	// FALLBACK[FB-003]: trigger=selected encoder becomes unavailable/disabled
	// observe=console.warn + temporary availability hint override
	// sunset=2026-04-30 issue=#198
	console.warn(`FALLBACK[FB-003] encoder '${from}' unavailable; auto-switched to '${to}'`);
	encoderPanelState.availabilityHint = `Encoder '${from}' unavailable; switched to '${to}'.`;

	if (fallbackHintTimer) {
		clearTimeout(fallbackHintTimer);
	}
	fallbackHintTimer = setTimeout(() => {
		updateAvailabilityHint();
		fallbackHintTimer = null;
	}, 3500);
};

const getDisabledEncoderOptions = (
	availability: EncoderAvailability | null,
): typeof encoderPanelState.disabledEncoderOptions => ({
	fdk_he_aac: !ENABLE_FDK || !availability?.fdkAvailable,
	aac_at: availability ? !availability.aacAtAvailable : false,
	native_aac: availability ? !availability.nativeAacAvailable : false,
});

const enforceAllowedEncoderSelection = (): void => {
	const disabledOptions = getDisabledEncoderOptions(encoderPanelState.availability);
	encoderPanelState.disabledEncoderOptions = disabledOptions;

	if (!disabledOptions[encoderPanelState.flavor as keyof typeof disabledOptions]) {
		return;
	}

	const fallbackOrder: EncoderFlavor[] = ['auto', 'fdk_he_aac', 'aac_at', 'native_aac'];
	const fallbackFlavor = fallbackOrder.find((flavor) => {
		if (flavor === 'auto') return true;
		return !disabledOptions[flavor];
	});

	if (!fallbackFlavor) {
		return;
	}

	const previousFlavor = encoderPanelState.flavor;
	encoderPanelState.flavor = fallbackFlavor;
	reportEncoderAutoSwitchFallback(previousFlavor, fallbackFlavor);
};

const syncEncoderState = (): void => {
	enforceAllowedEncoderSelection();
	const effectiveEncoder = resolveEffectiveEncoder(encoderPanelState.flavor);
	enforceBitrateModeCompatibility(effectiveEncoder);
	updateQualityVisibility();
	updateInlineOptions();
	updateProfileDisplay(effectiveEncoder);
	updateEstimatedBitrate();
	updateAutoOptionLabel();
	updateAvailabilityHint();
};

const syncAfterStateChange = (): void => {
	syncEncoderState();
	queuePersistState();
	syncOutputSizingFromEncoderState();
};

const hydrateAvailability = async (): Promise<void> => {
	try {
		const availability = await tauriClient.listAvailableEncoders();
		debugLog('Encoder availability:', availability);
		setEncoderAvailability(availability);
	} catch (error) {
		console.warn('Failed to load encoder availability', error);
		setEncoderAvailability(null);
	}

	syncEncoderState();
	syncOutputSizingFromEncoderState();
	debugLog('Encoder panel ready');
};

export const initializeEncoderPanelLogic = (): void => {
	debugLog('Initializing encoder panel...');
	applyPersistedEncoderState(loadState());
	resetAutoResolutionHints();
	syncOutputSizingFromEncoderState();
	queuePersistState();
	void hydrateAvailability();
};

export const handleFlavorChange = (event: Event): void => {
	const target = event.currentTarget as HTMLSelectElement | null;
	encoderPanelState.flavor = (target?.value as EncoderFlavor | undefined) ?? 'auto';
	syncAfterStateChange();
};

export const handleBitrateModeChange = (event: Event): void => {
	const target = event.currentTarget as HTMLSelectElement | null;
	const value = target?.value;
	if (value === 'vbr' || value === 'cvbr' || value === 'cbr') {
		encoderPanelState.bitrateModeSelection = value;
	}
	syncAfterStateChange();
};

export const handleChannelsSelectionChange = (event: Event): void => {
	const target = event.currentTarget as HTMLSelectElement | null;
	const value = target?.value;
	if (value === 'auto' || value === 'mono' || value === 'stereo') {
		encoderPanelState.channelsSelection = value;
	}
	syncAfterStateChange();
};

export const handleQualityValueChange = (event: Event): void => {
	const target = event.currentTarget as HTMLSelectElement | null;
	const value = Number.parseInt(target?.value ?? '3', 10);
	encoderPanelState.qualityValue = clamp(value, 1, 5) as VbrLevel;
	syncAfterStateChange();
};

export const handleBitrateValueChange = (event: Event): void => {
	const target = event.currentTarget as HTMLSelectElement | null;
	const value = Number.parseInt(target?.value ?? '64', 10);
	if (Number.isFinite(value)) {
		encoderPanelState.bitrateValue = value as typeof encoderPanelState.bitrateValue;
	}
	syncAfterStateChange();
};

export const handleFdkAfterburnerChange = (event: Event): void => {
	const target = event.currentTarget as HTMLInputElement | null;
	encoderPanelState.fdkAfterburner = Boolean(target?.checked);
	queuePersistState();
	syncOutputState();
};

export const handleNativeTwoloopChange = (event: Event): void => {
	const target = event.currentTarget as HTMLInputElement | null;
	encoderPanelState.nativeTwoloop = Boolean(target?.checked);
	queuePersistState();
	syncOutputState();
};

export const handleSampleRateSelectionChange = (event: Event): void => {
	const target = event.currentTarget as HTMLSelectElement | null;
	encoderPanelState.sampleRateSelection = target?.value ?? 'auto';
	syncAfterStateChange();
};
