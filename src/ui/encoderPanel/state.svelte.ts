import type { SampleRateConfig } from '../../types/audio';
import { VALID_ENCODER_BITRATES } from '../../types/audio';
import {
	type EncoderFlavor,
	type EncoderSettingsV2,
	toBoundaryEncoderSettings,
} from '../../types/encoder';

export type EncoderAvailability = {
	fdkAvailable: boolean;
	aacAtAvailable: boolean;
	nativeAacAvailable: boolean;
};

export type VbrLevel = 1 | 2 | 3 | 4 | 5;
export type BitrateModeSelection = 'vbr' | 'cvbr' | 'cbr';
export type EncoderModeAvailability = Record<BitrateModeSelection, boolean>;

const DEFAULT_SAMPLE_RATE_HINT = 'Auto resolves from source audio.';
const DEFAULT_CHANNELS_HINT = 'Auto resolves from source audio.';
const DEFAULT_AVAILABILITY_HINT = 'Checking encoder availability…';

const createDefaultState = () => ({
	flavor: 'auto' as EncoderFlavor,
	bitrateModeSelection: 'vbr' as BitrateModeSelection,
	sampleRateSelection: 'auto',
	channelsSelection: 'auto' as EncoderSettingsV2['channels'],
	qualityValue: 3 as VbrLevel,
	bitrateValue: 64 as EncoderSettingsV2['bitrateKbps'],
	fdkAfterburner: true,
	nativeTwoloop: true,
	availability: null as EncoderAvailability | null,
	autoOptionLabel: 'Auto',
	availabilityHint: DEFAULT_AVAILABILITY_HINT,
	profileDisplay: 'HE-AAC v1',
	qualityBitrateLabel: 'Quality',
	estimatedBitrateText: 'Est: ~60 kbps',
	sampleRateAutoHint: DEFAULT_SAMPLE_RATE_HINT,
	channelsAutoHint: DEFAULT_CHANNELS_HINT,
	showQuality: true,
	showInlineOptionRow: false,
	showFdkOptions: false,
	showNativeOptions: false,
	bitrateModeAvailability: {
		vbr: true,
		cvbr: false,
		cbr: false,
	} as EncoderModeAvailability,
	disabledEncoderOptions: {
		fdk_he_aac: false,
		aac_at: false,
		native_aac: false,
	},
});

export const encoderPanelState = $state(createDefaultState());

export function resetEncoderPanelState(): void {
	const defaults = createDefaultState();
	encoderPanelState.flavor = defaults.flavor;
	encoderPanelState.bitrateModeSelection = defaults.bitrateModeSelection;
	encoderPanelState.sampleRateSelection = defaults.sampleRateSelection;
	encoderPanelState.channelsSelection = defaults.channelsSelection;
	encoderPanelState.qualityValue = defaults.qualityValue;
	encoderPanelState.bitrateValue = defaults.bitrateValue;
	encoderPanelState.fdkAfterburner = defaults.fdkAfterburner;
	encoderPanelState.nativeTwoloop = defaults.nativeTwoloop;
	encoderPanelState.availability = defaults.availability;
	encoderPanelState.autoOptionLabel = defaults.autoOptionLabel;
	encoderPanelState.availabilityHint = defaults.availabilityHint;
	encoderPanelState.profileDisplay = defaults.profileDisplay;
	encoderPanelState.qualityBitrateLabel = defaults.qualityBitrateLabel;
	encoderPanelState.estimatedBitrateText = defaults.estimatedBitrateText;
	encoderPanelState.sampleRateAutoHint = defaults.sampleRateAutoHint;
	encoderPanelState.channelsAutoHint = defaults.channelsAutoHint;
	encoderPanelState.showQuality = defaults.showQuality;
	encoderPanelState.showInlineOptionRow = defaults.showInlineOptionRow;
	encoderPanelState.showFdkOptions = defaults.showFdkOptions;
	encoderPanelState.showNativeOptions = defaults.showNativeOptions;
	encoderPanelState.bitrateModeAvailability = defaults.bitrateModeAvailability;
	encoderPanelState.disabledEncoderOptions = defaults.disabledEncoderOptions;
}

export function readEncoderSettingsFromState(): EncoderSettingsV2 {
	return {
		flavor: encoderPanelState.flavor,
		channels: encoderPanelState.channelsSelection,
		bitrateKbps: encoderPanelState.bitrateValue,
		bitrateMode:
			encoderPanelState.bitrateModeSelection === 'vbr'
				? { mode: 'vbr', value: encoderPanelState.qualityValue }
				: { mode: encoderPanelState.bitrateModeSelection },
		vbr: { enabled: true, level: encoderPanelState.qualityValue },
		fdkAfterburner: encoderPanelState.fdkAfterburner,
		twoloop: encoderPanelState.nativeTwoloop,
	};
}

export function readPersistedEncoderState(): Partial<EncoderSettingsV2> {
	const settings = readEncoderSettingsFromState();
	return {
		flavor: settings.flavor,
		channels: settings.channels,
		bitrateKbps: settings.bitrateKbps,
		bitrateMode: settings.bitrateMode,
		vbr: settings.vbr,
		fdkAfterburner: settings.fdkAfterburner,
		twoloop: settings.twoloop,
	};
}

export function readBoundaryEncoderSettings() {
	return toBoundaryEncoderSettings(readEncoderSettingsFromState());
}

export function readSampleRateFromState(): SampleRateConfig {
	if (encoderPanelState.sampleRateSelection === 'auto') {
		return 'auto';
	}

	const parsedRate = Number.parseInt(encoderPanelState.sampleRateSelection, 10);
	if (Number.isFinite(parsedRate) && parsedRate > 0) {
		return { explicit: parsedRate };
	}

	return 'auto';
}

export function applyPersistedEncoderState(state: Partial<EncoderSettingsV2>): void {
	if (state.flavor) {
		encoderPanelState.flavor = state.flavor;
	}
	if (state.channels) {
		encoderPanelState.channelsSelection = state.channels;
	}
	if (
		typeof state.bitrateKbps === 'number' &&
		VALID_ENCODER_BITRATES.includes(state.bitrateKbps as EncoderSettingsV2['bitrateKbps'])
	) {
		encoderPanelState.bitrateValue = state.bitrateKbps as EncoderSettingsV2['bitrateKbps'];
	}
	if (state.bitrateMode) {
		encoderPanelState.bitrateModeSelection = state.bitrateMode.mode;
	}
	if (typeof state.vbr?.level === 'number') {
		const clamped = Math.max(1, Math.min(5, Math.round(state.vbr.level))) as VbrLevel;
		encoderPanelState.qualityValue = clamped;
	}
	if (state.fdkAfterburner !== undefined) {
		encoderPanelState.fdkAfterburner = state.fdkAfterburner;
	}
	if (state.twoloop !== undefined) {
		encoderPanelState.nativeTwoloop = state.twoloop;
	}
}

export function setEncoderAvailability(availability: EncoderAvailability | null): void {
	encoderPanelState.availability = availability;
}

export function setSampleRateAutoHint(value: string): void {
	encoderPanelState.sampleRateAutoHint = value;
}

export function setChannelsAutoHint(value: string): void {
	encoderPanelState.channelsAutoHint = value;
}

export function resetAutoHints(): void {
	encoderPanelState.sampleRateAutoHint = DEFAULT_SAMPLE_RATE_HINT;
	encoderPanelState.channelsAutoHint = DEFAULT_CHANNELS_HINT;
}
