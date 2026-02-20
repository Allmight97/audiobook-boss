import { ENABLE_FDK } from './featureFlags';
import { queryDom, type EncoderDomCache } from './dom';
import { loadState, saveState } from './state';
import type { EncoderSettingsLike, EncoderFlavor, EncoderSettingsV2 } from '../../types/encoder';
import { VALID_ENCODER_BITRATES } from '../../types/audio';
import { tauriClient } from '../../lib/tauri/client';
import { resetAutoResolutionHints } from './autoResolutionHints';

/** Debug logging - only active in development builds */
const DEBUG = import.meta.env.DEV;
const debugLog = (...args: unknown[]): void => {
	if (DEBUG) console.log('[EncoderPanel]', ...args);
};

type WindowWithEncoderProvider = Window & {
	EncoderSettingsProvider?: () => EncoderSettingsLike;
};

type EncoderAvailability = {
	fdkAvailable: boolean;
	aacAtAvailable: boolean;
	nativeAacAvailable: boolean;
};

type VbrLevel = 1 | 2 | 3 | 4 | 5;

const clamp = (value: number, min: number, max: number): number =>
	Math.min(max, Math.max(min, value));

let cachedAvailability: EncoderAvailability | null = null;
let fallbackHintTimer: ReturnType<typeof setTimeout> | null = null;

// DOM cache - initialized once, reused across all functions
let domCache: EncoderDomCache | null = null;

/** Returns cached DOM references, querying only once per session */
const ensureDomCache = (): EncoderDomCache => {
	if (!domCache) {
		domCache = queryDom();
	}
	return domCache;
};

// Debounce timer for localStorage writes
let persistTimer: ReturnType<typeof setTimeout> | null = null;
const PERSIST_DEBOUNCE_MS = 300;

/** VBR level to estimated bitrate mapping */
const VBR_BITRATE_ESTIMATES: Record<VbrLevel, number> = {
	1: 32,
	2: 48,
	3: 60,
	4: 72,
	5: 96,
};

/** Profile display names per encoder */
const ENCODER_PROFILES: Record<EncoderFlavor, string> = {
	auto: 'HE-AAC v1',
	fdk_he_aac: 'HE-AAC v1',
	aac_at: 'AAC-LC',
	native_aac: 'AAC-LC',
};
const NATIVE_AAC_WARNING =
	'Native AAC may sound degraded on speech (known issue; prefer Auto/Apple/FDK).';

/** Resolves effective encoder when "auto" is selected */
const resolveEffectiveEncoder = (flavor: EncoderFlavor): EncoderFlavor => {
	if (flavor !== 'auto') return flavor;
	if (cachedAvailability?.fdkAvailable && ENABLE_FDK) return 'fdk_he_aac';
	if (cachedAvailability?.aacAtAvailable) return 'aac_at';
	return 'native_aac';
};

/**
 * Reads the current encoder settings from the DOM
 */
const getEncoderSettingsFromDom = (): EncoderSettingsLike => {
	const dom = ensureDomCache();
	if (!dom.root) return undefined;

	const encoderValue = dom.encoderSelect?.value as EncoderFlavor | undefined;
	const flavor: EncoderFlavor = encoderValue ?? 'auto';

	const channelsValue = dom.channelsSelect?.value ?? 'auto';
	const channels: EncoderSettingsV2['channels'] =
		channelsValue === 'stereo' ? 'stereo' : channelsValue === 'mono' ? 'mono' : 'auto';

	// Read bitrate from bitrate select (for CVBR/CBR)
	const bitrateValue = parseInt(dom.bitrateSelect?.value ?? '64', 10);
	const bitrateKbps = (
		[...VALID_ENCODER_BITRATES].includes(bitrateValue as (typeof VALID_ENCODER_BITRATES)[number])
			? bitrateValue
			: 64
	) as EncoderSettingsV2['bitrateKbps'];

	// Read VBR level from quality select
	const qualityValue = parseInt(dom.qualitySelect?.value ?? '3', 10);
	const vbrLevel = clamp(qualityValue, 1, 5) as VbrLevel;

	const bitrateModeValue = dom.bitrateModeSelect?.value ?? 'vbr';
	const bitrateMode =
		bitrateModeValue === 'cbr'
			? { mode: 'cbr' as const }
			: bitrateModeValue === 'cvbr'
				? { mode: 'cvbr' as const }
				: { mode: 'vbr' as const, value: vbrLevel };

	const effectiveEncoder = resolveEffectiveEncoder(flavor);
	const vbr = effectiveEncoder === 'fdk_he_aac' ? { enabled: true, level: vbrLevel } : undefined;
	const fdkAfterburner =
		effectiveEncoder === 'fdk_he_aac' ? !!dom.fdkAfterburner?.checked : undefined;

	return {
		flavor,
		channels,
		bitrateKbps,
		bitrateMode,
		vbr,
		fdkAfterburner,
		twoloop: dom.nativeTwoloop ? !!dom.nativeTwoloop.checked : true,
	};
};

export const initializeEncoderPanelLogic = (): void => {
	debugLog('Initializing encoder panel...');

	// Initialize DOM cache
	domCache = queryDom();
	const dom = ensureDomCache();
	if (!dom.root) {
		debugLog('Panel not present in this view, skipping initialization');
		return;
	}

	applyPersistedState();
	resetAutoResolutionHints(dom);
	hydrateAvailability().finally(() => {
		syncEncoderUI();
		persistState();
		debugLog('Encoder panel ready');
	});
	attachEventListeners();
	(window as WindowWithEncoderProvider).EncoderSettingsProvider = getEncoderSettingsFromDom;
};

const applyPersistedState = (): void => {
	const state = loadState();
	const dom = ensureDomCache();

	if (state.flavor && dom.encoderSelect) {
		dom.encoderSelect.value = state.flavor;
	}
	if (state.channels && dom.channelsSelect) {
		dom.channelsSelect.value =
			state.channels === 'stereo' ? 'stereo' : state.channels === 'mono' ? 'mono' : 'auto';
	}
	if (state.bitrateKbps && dom.bitrateSelect) {
		dom.bitrateSelect.value = String(state.bitrateKbps);
	}
	if (state.bitrateMode && dom.bitrateModeSelect) {
		dom.bitrateModeSelect.value = state.bitrateMode.mode;
	}
	if (state.vbr?.level && dom.qualitySelect) {
		dom.qualitySelect.value = String(state.vbr.level);
	}
	if (state.fdkAfterburner !== undefined && dom.fdkAfterburner) {
		dom.fdkAfterburner.checked = state.fdkAfterburner;
	}
	if (state.twoloop !== undefined && dom.nativeTwoloop) {
		dom.nativeTwoloop.checked = state.twoloop;
	}
};

const attachEventListeners = (): void => {
	const dom = ensureDomCache();

	dom.encoderSelect?.addEventListener('change', () => {
		syncEncoderUI();
		persistState();
	});
	dom.bitrateModeSelect?.addEventListener('change', () => {
		syncQualityBitrateVisibility();
		updateEstimatedBitrate();
		persistState();
	});
	dom.channelsSelect?.addEventListener('change', persistState);
	dom.qualitySelect?.addEventListener('change', () => {
		updateEstimatedBitrate();
		persistState();
	});
	dom.bitrateSelect?.addEventListener('change', () => {
		updateEstimatedBitrate();
		persistState();
	});
	dom.fdkAfterburner?.addEventListener('change', persistState);
	dom.nativeTwoloop?.addEventListener('change', persistState);
};

/** Debounced state persistence to prevent localStorage thrashing */
const persistState = (): void => {
	if (persistTimer) {
		clearTimeout(persistTimer);
	}
	persistTimer = setTimeout(() => {
		const settings = getEncoderSettingsFromDom();
		if (settings) {
			saveState(settings);
		}
		persistTimer = null;
	}, PERSIST_DEBOUNCE_MS);
};

const hydrateAvailability = async (): Promise<void> => {
	try {
		cachedAvailability = await tauriClient.listAvailableEncoders();
		debugLog('Encoder availability:', cachedAvailability);
	} catch (error) {
		console.warn('Failed to load encoder availability', error);
		cachedAvailability = null;
	}
	updateAvailabilityHint();
};

const updateAvailabilityHint = (): void => {
	const dom = ensureDomCache();
	if (!dom.encoderAvailabilityHint) return;

	if (!cachedAvailability) {
		dom.encoderAvailabilityHint.textContent = 'Checking encoder availability…';
		return;
	}

	const selectedFlavor = (dom.encoderSelect?.value as EncoderFlavor | undefined) ?? 'auto';
	const effectiveEncoder = resolveEffectiveEncoder(selectedFlavor);
	if (effectiveEncoder === 'native_aac') {
		dom.encoderAvailabilityHint.textContent = NATIVE_AAC_WARNING;
		return;
	}

	// Single concise line showing what's available
	if (cachedAvailability.fdkAvailable && ENABLE_FDK) {
		dom.encoderAvailabilityHint.textContent = 'FDK detected ✓';
	} else if (cachedAvailability.aacAtAvailable) {
		dom.encoderAvailabilityHint.textContent = 'Apple AAC available';
	} else {
		dom.encoderAvailabilityHint.textContent = 'Using native encoder';
	}
};

const reportEncoderAutoSwitchFallback = (from: EncoderFlavor, to: EncoderFlavor): void => {
	// FALLBACK[FB-003]: trigger=selected encoder becomes unavailable/disabled
	// observe=console.warn + temporary availability hint override
	// sunset=2026-04-30 issue=#198
	console.warn(`FALLBACK[FB-003] encoder '${from}' unavailable; auto-switched to '${to}'`);

	const dom = ensureDomCache();
	if (!dom.encoderAvailabilityHint) return;
	dom.encoderAvailabilityHint.textContent = `Encoder '${from}' unavailable; switched to '${to}'.`;

	if (fallbackHintTimer) {
		clearTimeout(fallbackHintTimer);
	}
	fallbackHintTimer = setTimeout(() => {
		updateAvailabilityHint();
		fallbackHintTimer = null;
	}, 3500);
};

/** Main sync function - updates all encoder-dependent UI */
const syncEncoderUI = (): void => {
	const dom = ensureDomCache();
	const flavor = (dom.encoderSelect?.value as EncoderFlavor | undefined) ?? 'auto';
	const effectiveEncoder = resolveEffectiveEncoder(flavor);

	updateProfileDisplay(effectiveEncoder);
	enforceBitrateModeCompatibility(effectiveEncoder);
	syncQualityBitrateVisibility();
	syncEncoderOptions(effectiveEncoder);
	updateEstimatedBitrate();
	disableDisallowedEncoders();
	updateAvailabilityHint();
};

const updateProfileDisplay = (encoder: EncoderFlavor): void => {
	const dom = ensureDomCache();
	if (!dom.profileDisplay) return;

	dom.profileDisplay.textContent = ENCODER_PROFILES[encoder] ?? 'AAC-LC';
};

const enforceBitrateModeCompatibility = (encoder: EncoderFlavor): void => {
	const dom = ensureDomCache();
	const select = dom.bitrateModeSelect;
	if (!select) return;

	// Enable all options first
	for (const opt of select.querySelectorAll('option')) {
		opt.disabled = false;
	}

	// Lock to encoder-specific mode
	if (encoder === 'aac_at') {
		select.value = 'cvbr';
		for (const opt of select.querySelectorAll('option')) {
			if (opt.value !== 'cvbr') opt.disabled = true;
		}
	} else if (encoder === 'native_aac') {
		select.value = 'cbr';
		for (const opt of select.querySelectorAll('option')) {
			if (opt.value !== 'cbr') opt.disabled = true;
		}
	} else {
		// FDK or auto (resolves to FDK)
		select.value = 'vbr';
		for (const opt of select.querySelectorAll('option')) {
			if (opt.value !== 'vbr') opt.disabled = true;
		}
	}
};

const syncQualityBitrateVisibility = (): void => {
	const dom = ensureDomCache();
	const mode = dom.bitrateModeSelect?.value ?? 'vbr';

	const showQuality = mode === 'vbr';
	if (dom.qualitySelect) {
		dom.qualitySelect.classList.toggle('hidden', !showQuality);
	}
	if (dom.bitrateSelect) {
		dom.bitrateSelect.classList.toggle('hidden', showQuality);
	}
	if (dom.qualityBitrateLabel) {
		dom.qualityBitrateLabel.textContent = showQuality ? 'Quality' : 'Bitrate';
	}
};

const syncEncoderOptions = (encoder: EncoderFlavor): void => {
	const dom = ensureDomCache();

	// Show/hide encoder-specific option groups
	if (dom.fdkOptions) {
		dom.fdkOptions.classList.toggle('hidden', encoder !== 'fdk_he_aac');
	}
	if (dom.nativeOptions) {
		dom.nativeOptions.classList.toggle('hidden', encoder !== 'native_aac');
	}
	if (dom.appleOptions) {
		dom.appleOptions.classList.toggle('hidden', encoder !== 'aac_at');
	}

	// Enable/disable afterburner based on FDK feature flag
	if (dom.fdkAfterburner) {
		dom.fdkAfterburner.disabled = !ENABLE_FDK;
	}
};

const updateEstimatedBitrate = (): void => {
	const dom = ensureDomCache();
	if (!dom.estimatedBitrate) return;

	const mode = dom.bitrateModeSelect?.value ?? 'vbr';

	if (mode === 'vbr') {
		const level = clamp(parseInt(dom.qualitySelect?.value ?? '3', 10), 1, 5) as VbrLevel;
		const estimate = VBR_BITRATE_ESTIMATES[level];
		dom.estimatedBitrate.textContent = `Est: ~${estimate} kbps`;
	} else {
		// CVBR or CBR - show target bitrate
		const bitrate = dom.bitrateSelect?.value ?? '64';
		dom.estimatedBitrate.textContent = `Target: ${bitrate} kbps`;
	}
};

const disableDisallowedEncoders = (): void => {
	const dom = ensureDomCache();
	const select = dom.encoderSelect;
	if (!select) return;

	const availability = cachedAvailability;

	Array.from(select.options).forEach((option) => {
		switch (option.value) {
			case 'fdk_he_aac':
				option.disabled = !ENABLE_FDK || !availability?.fdkAvailable;
				break;
			case 'aac_at':
				option.disabled = availability ? !availability.aacAtAvailable : false;
				break;
			case 'native_aac':
				option.disabled = availability ? !availability.nativeAacAvailable : false;
				break;
			default:
				option.disabled = false;
		}
	});

	// Fall back if current selection is disabled
	if (select.selectedOptions[0]?.disabled) {
		const from = select.value as EncoderFlavor;
		const fallback = Array.from(select.options).find((opt) => !opt.disabled);
		if (fallback) {
			select.value = fallback.value;
			reportEncoderAutoSwitchFallback(from, fallback.value as EncoderFlavor);
			syncEncoderUI(); // Re-sync after fallback
		}
	}
};
