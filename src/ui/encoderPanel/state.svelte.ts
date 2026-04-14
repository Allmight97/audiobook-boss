import type { EncoderAvailability, SampleRateConfig } from '../../types/audio';
import { VALID_ENCODER_BITRATES } from '../../types/audio';
import {
	type EncoderFlavor,
	type EncoderSettingsState,
	toBoundaryEncoderSettings,
} from '../../types/encoder';

export type VbrLevel = 1 | 2 | 3 | 4 | 5;
export type BitrateModeSelection = 'vbr' | 'cvbr' | 'cbr';
export type EncoderModeAvailability = Record<BitrateModeSelection, boolean>;

const DEFAULT_SAMPLE_RATE_HINT = 'Auto resolves from source audio.';
const DEFAULT_CHANNELS_HINT = 'Auto resolves from source audio.';
const DEFAULT_AVAILABILITY_HINT = 'Checking encoder availability…';
const DEFAULT_TOOLCHAIN_TITLE = 'Checking FDK AAC';
const DEFAULT_TOOLCHAIN_MESSAGE = 'Looking for an external FFmpeg build with libfdk_aac.';

export interface PersistedEncoderPanelState {
	flavor?: EncoderSettingsState['flavor'];
	channels?: EncoderSettingsState['channels'];
	bitrateKbps?: EncoderSettingsState['bitrateKbps'];
	bitrateMode?: EncoderSettingsState['bitrateMode'];
	externalToolchainOverridePath?: string;
	externalToolchain?: {
		overridePath?: string;
		mode?: 'auto' | 'custom';
		customPath?: string;
	};
	vbr?: EncoderSettingsState['vbr'];
	fdkAfterburner?: EncoderSettingsState['fdkAfterburner'];
	twoloop?: EncoderSettingsState['twoloop'];
}

const createDefaultState = () => ({
	flavor: 'auto' as EncoderFlavor,
	bitrateModeSelection: 'vbr' as BitrateModeSelection,
	sampleRateSelection: 'auto',
	channelsSelection: 'auto' as EncoderSettingsState['channels'],
	qualityValue: 3 as VbrLevel,
	bitrateValue: 64 as EncoderSettingsState['bitrateKbps'],
	fdkAfterburner: true,
	nativeTwoloop: true,
	externalToolchainOverridePath: '',
	availability: null as EncoderAvailability | null,
	autoOptionLabel: 'Auto',
	availabilityHint: DEFAULT_AVAILABILITY_HINT,
	profileDisplay: 'HE-AAC v1',
	qualityBitrateLabel: 'Quality',
	estimatedBitrateText: 'Est: ~60 kbps',
	sampleRateAutoHint: DEFAULT_SAMPLE_RATE_HINT,
	channelsAutoHint: DEFAULT_CHANNELS_HINT,
	toolchainStatusTitle: DEFAULT_TOOLCHAIN_TITLE,
	toolchainStatusMessage: DEFAULT_TOOLCHAIN_MESSAGE,
	toolchainDetectedPath: '',
	toolchainActivePath: '',
	toolchainOverrideError: '',
	showQuality: true,
	showInlineOptionRow: false,
	showFdkOptions: false,
	showNativeOptions: false,
	showToolchainOverrideInput: false,
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
	Object.assign(encoderPanelState, defaults);
}

export function readToolchainSettingsFromState() {
	const overridePath = encoderPanelState.externalToolchainOverridePath.trim();
	return overridePath ? { overridePath } : {};
}

export function readEncoderSettingsFromState(): EncoderSettingsState {
	return {
		flavor: encoderPanelState.flavor,
		channels: encoderPanelState.channelsSelection,
		bitrateKbps: encoderPanelState.bitrateValue,
		bitrateMode:
			encoderPanelState.bitrateModeSelection === 'vbr'
				? { mode: 'vbr', value: encoderPanelState.qualityValue }
				: { mode: encoderPanelState.bitrateModeSelection },
		externalToolchain: readToolchainSettingsFromState(),
		vbr: { enabled: true, level: encoderPanelState.qualityValue },
		fdkAfterburner: encoderPanelState.fdkAfterburner,
		twoloop: encoderPanelState.nativeTwoloop,
	};
}

export function readPersistedEncoderState(): PersistedEncoderPanelState {
	const settings = readEncoderSettingsFromState();
	return {
		flavor: settings.flavor,
		channels: settings.channels,
		bitrateKbps: settings.bitrateKbps,
		bitrateMode: settings.bitrateMode,
		externalToolchainOverridePath: settings.externalToolchain?.overridePath,
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

function migrateOverridePath(state: PersistedEncoderPanelState): string {
	const explicitOverride = state.externalToolchainOverridePath?.trim();
	if (explicitOverride) {
		return explicitOverride;
	}

	const nestedOverride = state.externalToolchain?.overridePath?.trim();
	if (nestedOverride) {
		return nestedOverride;
	}

	if (state.externalToolchain?.mode === 'custom') {
		return state.externalToolchain.customPath?.trim() ?? '';
	}

	return '';
}

export function applyPersistedEncoderState(state: PersistedEncoderPanelState): void {
	if (state.flavor) {
		encoderPanelState.flavor = state.flavor;
	}
	if (state.channels) {
		encoderPanelState.channelsSelection = state.channels;
	}
	if (
		typeof state.bitrateKbps === 'number' &&
		VALID_ENCODER_BITRATES.includes(state.bitrateKbps as EncoderSettingsState['bitrateKbps'])
	) {
		encoderPanelState.bitrateValue = state.bitrateKbps as EncoderSettingsState['bitrateKbps'];
	}
	if (state.bitrateMode) {
		encoderPanelState.bitrateModeSelection = state.bitrateMode.mode;
	}
	encoderPanelState.externalToolchainOverridePath = migrateOverridePath(state);
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
	if (!availability) {
		encoderPanelState.toolchainStatusTitle = DEFAULT_TOOLCHAIN_TITLE;
		encoderPanelState.toolchainStatusMessage = DEFAULT_TOOLCHAIN_MESSAGE;
		encoderPanelState.toolchainDetectedPath = '';
		encoderPanelState.toolchainActivePath = '';
		encoderPanelState.toolchainOverrideError = '';
		encoderPanelState.showToolchainOverrideInput =
			encoderPanelState.externalToolchainOverridePath.trim().length > 0;
		return;
	}

	encoderPanelState.toolchainDetectedPath = availability.detectedToolchainPath ?? '';
	encoderPanelState.toolchainActivePath =
		availability.activeToolchainPath ??
		availability.detectedToolchainPath ??
		availability.overrideToolchainPath ??
		'';
	encoderPanelState.toolchainOverrideError = availability.overrideError ?? '';
	encoderPanelState.showToolchainOverrideInput =
		availability.overrideInvalid || !availability.fdkAvailable;

	if (availability.fdkAvailable) {
		encoderPanelState.toolchainStatusTitle =
			availability.fdkSource === 'override' ? 'Using override path' : 'FDK detected';
	} else if (availability.overrideInvalid) {
		encoderPanelState.toolchainStatusTitle = 'Saved override path is invalid';
	} else {
		encoderPanelState.toolchainStatusTitle = 'FDK not found';
	}

	encoderPanelState.toolchainStatusMessage = availability.statusMessage;
}

export function setExternalToolchainOverridePath(path: string): void {
	encoderPanelState.externalToolchainOverridePath = path;
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
