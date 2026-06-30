import type {
	EncoderAvailability,
	EncoderSettings,
	EncoderSettingsCapabilities,
	EncodingRequestConfig,
	SampleRateConfig,
} from '../../types/audio';
import type { EncoderDefaults } from '../../types/appSettings';
import {
	type EncoderFlavor,
	type EncoderSettingsState,
	toBoundaryEncoderSettings,
} from '../../types/encoder';

export type VbrLevel = number;
export type BitrateModeSelection = 'vbr' | 'cvbr' | 'cbr';
export type EncoderModeAvailability = Record<BitrateModeSelection, boolean>;

const DEFAULT_SAMPLE_RATE_HINT = 'Auto -> source audio';
const DEFAULT_CHANNELS_HINT = 'Auto -> source audio';
const DEFAULT_AVAILABILITY_HINT = 'Checking encoder availability…';
const createDefaultState = () => ({
	flavor: 'auto' as EncoderFlavor,
	bitrateModeSelection: 'vbr' as BitrateModeSelection,
	sampleRateSelection: 'auto',
	channelsSelection: 'auto' as EncoderSettingsState['channels'],
	qualityValue: 3 as VbrLevel,
	bitrateValue: 64 as EncoderSettingsState['bitrateKbps'],
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
	capabilities: null as EncoderSettingsCapabilities | null,
	encoderOptions: [] as EncoderFlavor[],
	bitrateModeOptions: [] as BitrateModeSelection[],
	qualityOptions: [] as number[],
	bitrateOptions: [] as number[],
	sampleRateOptions: [] as string[],
	channelOptions: [] as NonNullable<EncoderSettingsState['channels']>[],
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
	Object.assign(encoderPanelState, defaults);
}

function bitrateModeSelectionFromSettings(settings: EncoderSettings): BitrateModeSelection {
	switch (settings.bitrateMode.mode) {
		case 'cvbr':
			return 'cvbr';
		case 'cbr':
			return 'cbr';
		default:
			return 'vbr';
	}
}

function qualityValueFromSettings(settings: EncoderSettings): VbrLevel {
	if (settings.bitrateMode.mode !== 'vbr') {
		return encoderPanelState.capabilities?.vbrLevelDefault ?? 3;
	}

	const value = Number(
		settings.bitrateMode.value ?? encoderPanelState.capabilities?.vbrLevelDefault ?? 3,
	);
	const rounded = Math.round(value);
	if (!encoderPanelState.capabilities) {
		return rounded;
	}
	return Math.min(
		encoderPanelState.capabilities.vbrLevelMax,
		Math.max(encoderPanelState.capabilities.vbrLevelMin, rounded),
	);
}

function sampleRateSelectionFromConfig(
	sampleRate: SampleRateConfig,
	capabilities: EncoderSettingsCapabilities | null,
): string {
	if (sampleRate === 'auto') {
		return 'auto';
	}

	if (!capabilities) {
		return String(sampleRate.explicit);
	}

	return capabilities.explicitSampleRates.includes(sampleRate.explicit)
		? String(sampleRate.explicit)
		: 'auto';
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
		fdkAfterburner: encoderPanelState.fdkAfterburner,
		twoloop: encoderPanelState.nativeTwoloop,
	};
}

export function readBoundaryEncoderSettings() {
	return toBoundaryEncoderSettings(
		readEncoderSettingsFromState(),
		undefined,
		encoderPanelState.capabilities,
	);
}

export function readSampleRateFromState(): SampleRateConfig {
	if (encoderPanelState.sampleRateSelection === 'auto') {
		return 'auto';
	}

	const parsedRate = Number.parseInt(encoderPanelState.sampleRateSelection, 10);
	if (!Number.isFinite(parsedRate)) {
		return 'auto';
	}

	if (!encoderPanelState.capabilities) {
		return { explicit: parsedRate };
	}

	if (encoderPanelState.capabilities.explicitSampleRates.includes(parsedRate)) {
		return { explicit: parsedRate };
	}

	return 'auto';
}

export function readEncodingRequestConfig(): EncodingRequestConfig {
	return {
		encoderSettings: readBoundaryEncoderSettings(),
		sampleRate: readSampleRateFromState(),
	};
}

export function readEncoderDefaultsFromState(): EncoderDefaults {
	return {
		settings: readBoundaryEncoderSettings(),
		sampleRate: readSampleRateFromState(),
	};
}

export function applyEncoderDefaults(defaults: EncoderDefaults): void {
	const settings = toBoundaryEncoderSettings(
		defaults.settings,
		undefined,
		encoderPanelState.capabilities,
	);
	encoderPanelState.flavor = settings.encoderType;
	encoderPanelState.bitrateModeSelection = bitrateModeSelectionFromSettings(settings);
	encoderPanelState.qualityValue = qualityValueFromSettings(settings);
	encoderPanelState.bitrateValue = settings.bitrateKbps as typeof encoderPanelState.bitrateValue;
	encoderPanelState.channelsSelection = settings.channels;
	encoderPanelState.fdkAfterburner = settings.afterburner;
	encoderPanelState.nativeTwoloop = settings.twoloop ?? true;
	encoderPanelState.sampleRateSelection = sampleRateSelectionFromConfig(
		defaults.sampleRate,
		encoderPanelState.capabilities,
	);
}

function rangeOptions(min: number, max: number): number[] {
	const values: number[] = [];
	for (let value = min; value <= max; value += 1) {
		values.push(value);
	}
	return values;
}

function bitrateModeSelectionFromKind(kind: string): BitrateModeSelection {
	return kind === 'cvbr' || kind === 'cbr' ? kind : 'vbr';
}

export function setEncoderSettingsCapabilities(
	capabilities: EncoderSettingsCapabilities | null,
): void {
	encoderPanelState.capabilities = capabilities;
	encoderPanelState.encoderOptions = capabilities?.encoderTypes ?? [];
	encoderPanelState.bitrateModeOptions = [
		...new Set(
			(capabilities?.bitrateModesByEncoder ?? []).flatMap((entry) =>
				entry.allowedModes.map(bitrateModeSelectionFromKind),
			),
		),
	];
	encoderPanelState.qualityOptions = capabilities
		? rangeOptions(capabilities.vbrLevelMin, capabilities.vbrLevelMax)
		: [];
	encoderPanelState.bitrateOptions = capabilities?.bitrateKbpsOptions ?? [];
	encoderPanelState.sampleRateOptions = capabilities
		? [
				...(capabilities.sampleRateAuto ? ['auto'] : []),
				...capabilities.explicitSampleRates.map(String),
			]
		: [];
	encoderPanelState.channelOptions = capabilities?.channelOptions ?? [];
	if (capabilities) {
		if (!capabilities.bitrateKbpsOptions.includes(encoderPanelState.bitrateValue)) {
			encoderPanelState.bitrateValue = (capabilities.bitrateKbpsOptions[0] ??
				encoderPanelState.bitrateValue) as typeof encoderPanelState.bitrateValue;
		}
		encoderPanelState.qualityValue = Math.min(
			capabilities.vbrLevelMax,
			Math.max(capabilities.vbrLevelMin, encoderPanelState.qualityValue),
		) as VbrLevel;
		if (
			encoderPanelState.sampleRateSelection !== 'auto' &&
			!capabilities.explicitSampleRates.map(String).includes(encoderPanelState.sampleRateSelection)
		) {
			encoderPanelState.sampleRateSelection = capabilities.sampleRateAuto
				? 'auto'
				: (encoderPanelState.sampleRateOptions[0] ?? 'auto');
		}
		if (
			encoderPanelState.channelsSelection &&
			!capabilities.channelOptions.includes(encoderPanelState.channelsSelection)
		) {
			encoderPanelState.channelsSelection =
				capabilities.channelOptions[0] ?? encoderPanelState.channelsSelection;
		}
	}
	setEncoderAvailability(capabilities?.availability ?? null);
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
