import { updateEstimatedSize } from '../outputPanel/preview';
import {
	applyEncoderDefaults,
	encoderPanelState,
	readEncoderDefaultsFromState,
	setChannelsAutoHint,
	setExternalToolchainOverridePath,
	setSampleRateAutoHint,
	type BitrateModeSelection,
	type VbrLevel,
} from './state.svelte';
import type { EncoderFlavor } from '../../types/encoder';
import type { EncoderAvailability } from '../../types/audio';
import type { EncoderDefaults } from '../../types/appSettings';
import { resetAutoResolutionHints } from './autoResolutionHints';
import { runToolchainValidationWorkflow } from './toolchainValidationWorkflow';
import { persistEncoderDefaults } from '../appSettings/persistence';

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

const clamp = (value: number, min: number, max: number): number =>
	Math.min(max, Math.max(min, value));

const resolveEffectiveEncoder = (flavor: EncoderFlavor): EncoderFlavor => {
	if (flavor !== 'auto') return flavor;
	if (encoderPanelState.availability?.fdkAvailable) return 'fdk_he_aac';
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
			const pathSuffix = encoderPanelState.toolchainActivePath
				? ` at ${encoderPanelState.toolchainActivePath}.`
				: '.';
			encoderPanelState.availabilityHint = `Auto will use external FDK AAC${pathSuffix}`;
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
				'FDK AAC requires a validated external FFmpeg toolchain.';
			return;
		}
		encoderPanelState.availabilityHint =
			encoderPanelState.availability.fdkSource === 'override'
				? 'FDK AAC is using the saved override path.'
				: 'External FDK toolchain detected and ready.';
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
	updateInlineOptions();
	updateProfileDisplay(effectiveEncoder);
	updateEstimatedBitrate();
	updateAutoOptionLabel();
	updateAvailabilityHint();
};

export const syncAfterStateChange = (): void => {
	syncEncoderState();
	syncOutputSizingFromEncoderState();
};

const readAcceptedEncoderDefaultsFromState = (): EncoderDefaults => {
	const defaults = readEncoderDefaultsFromState();
	if (
		encoderPanelState.externalToolchainOverridePath.trim() &&
		encoderPanelState.availability?.overrideInvalid
	) {
		return {
			...defaults,
			externalToolchain: {},
		};
	}
	return defaults;
};

const persistCurrentEncoderDefaults = (): void => {
	void persistEncoderDefaults(readAcceptedEncoderDefaultsFromState());
};

export const syncEncoderPanelAfterAvailabilityChange = (): void => {
	syncEncoderState();
	syncOutputSizingFromEncoderState();
	debugLog('Encoder panel ready');
};

export const initializeEncoderPanelLogic = (): void => {
	debugLog('Initializing encoder panel...');
	resetAutoResolutionHints();
	setSampleRateAutoHint('Auto resolves from source audio.');
	setChannelsAutoHint('Auto resolves from source audio.');
	syncOutputSizingFromEncoderState();
	void runToolchainValidationWorkflow({ type: 'hydrateAvailability' });
};

export const applyEncodingDefaults = (defaults: EncoderDefaults): void => {
	applyEncoderDefaults(defaults);
	syncAfterStateChange();
	void runToolchainValidationWorkflow({ type: 'hydrateAvailability' });
};

export const handleFlavorChange = (event: Event): void => {
	const target = event.currentTarget as HTMLSelectElement | null;
	encoderPanelState.flavor = (target?.value as EncoderFlavor | undefined) ?? 'auto';
	syncAfterStateChange();
	persistCurrentEncoderDefaults();
};

export const handleBitrateModeChange = (event: Event): void => {
	const target = event.currentTarget as HTMLSelectElement | null;
	const value = target?.value;
	if (value === 'vbr' || value === 'cvbr' || value === 'cbr') {
		encoderPanelState.bitrateModeSelection = value;
	}
	syncAfterStateChange();
	persistCurrentEncoderDefaults();
};

export const handleChannelsSelectionChange = (event: Event): void => {
	const target = event.currentTarget as HTMLSelectElement | null;
	const value = target?.value;
	if (value === 'auto' || value === 'mono' || value === 'stereo') {
		encoderPanelState.channelsSelection = value;
	}
	syncAfterStateChange();
	persistCurrentEncoderDefaults();
};

export const handleQualityValueChange = (event: Event): void => {
	const target = event.currentTarget as HTMLSelectElement | null;
	const value = Number.parseInt(target?.value ?? '3', 10);
	encoderPanelState.qualityValue = clamp(value, 1, 5) as VbrLevel;
	syncAfterStateChange();
	persistCurrentEncoderDefaults();
};

export const handleBitrateValueChange = (event: Event): void => {
	const target = event.currentTarget as HTMLSelectElement | null;
	const value = Number.parseInt(target?.value ?? '64', 10);
	if (Number.isFinite(value)) {
		encoderPanelState.bitrateValue = value as typeof encoderPanelState.bitrateValue;
	}
	syncAfterStateChange();
	persistCurrentEncoderDefaults();
};

export const handleFdkAfterburnerChange = (event: Event): void => {
	const target = event.currentTarget as HTMLInputElement | null;
	encoderPanelState.fdkAfterburner = Boolean(target?.checked);
	persistCurrentEncoderDefaults();
};

export const handleNativeTwoloopChange = (event: Event): void => {
	const target = event.currentTarget as HTMLInputElement | null;
	encoderPanelState.nativeTwoloop = Boolean(target?.checked);
	persistCurrentEncoderDefaults();
};

export const handleToolchainPathInput = (event: Event): void => {
	const target = event.currentTarget as HTMLInputElement | null;
	setExternalToolchainOverridePath(target?.value ?? '');
};

export const handleToolchainPathCommit = (): void => {
	syncAfterStateChange();
	void runToolchainValidationWorkflow({ type: 'commitOverride' }).then(() => {
		persistCurrentEncoderDefaults();
	});
};

export const handleToolchainBrowse = async (): Promise<void> => {
	await runToolchainValidationWorkflow({ type: 'browseToolchain' });
	persistCurrentEncoderDefaults();
};

export const clearToolchainOverride = (): void => {
	void runToolchainValidationWorkflow({ type: 'clearOverride' }).then(() => {
		persistCurrentEncoderDefaults();
	});
};

export const refreshExternalToolchain = (): void => {
	void runToolchainValidationWorkflow({ type: 'refresh' });
};

export const handleSampleRateSelectionChange = (event: Event): void => {
	const target = event.currentTarget as HTMLSelectElement | null;
	encoderPanelState.sampleRateSelection = target?.value ?? 'auto';
	syncAfterStateChange();
	persistCurrentEncoderDefaults();
};
