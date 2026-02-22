/**
 * Output panel state management
 */
import type {
	EncoderSettings,
	OutputConfig,
	OutputNamingConfig,
	SampleRateConfig,
} from '../../types/audio';
import { defaultEncoderSettings } from '../../types/audio';
import { publishOutputDraft } from '../core/appStore.svelte';

export type OutputNamingPreset = OutputNamingConfig['preset'];
const DEFAULT_CUSTOM_TEMPLATE = '{author}/{title}';

/**
 * State interface for output panel configuration
 */
export interface OutputPanelState {
	encoderSettings: EncoderSettings;
	sampleRate: SampleRateConfig;
	outputDirectory: string;
	namingPreset: OutputNamingPreset;
	namingTemplate: string;
	absIncludeYear: boolean;
}

/**
 * Current output panel state (module-level singleton)
 */
const currentState: OutputPanelState = {
	encoderSettings: { ...defaultEncoderSettings() },
	sampleRate: { explicit: 22050 },
	outputDirectory: '',
	namingPreset: 'absDefault',
	namingTemplate: '',
	absIncludeYear: false,
};

/**
 * Gets the current output panel state
 */
export function getState(): OutputPanelState {
	return currentState;
}

/**
 * Builds output naming configuration from current state
 */
export function getOutputNamingConfig(): OutputNamingConfig {
	const trimmedTemplate = currentState.namingTemplate.trim();
	return {
		preset: currentState.namingPreset,
		includeYear: currentState.absIncludeYear,
		customTemplate:
			currentState.namingPreset === 'customTemplate'
				? trimmedTemplate.length > 0
					? currentState.namingTemplate
					: DEFAULT_CUSTOM_TEMPLATE
				: undefined,
	};
}

/**
 * Reads the canonical output configuration for processing.
 */
export function readOutputConfigForProcessing(): OutputConfig {
	if (!currentState.outputDirectory) {
		throw new Error('Output directory not selected');
	}

	return {
		encoderSettings: currentState.encoderSettings,
		sampleRate: currentState.sampleRate,
		outputPath: currentState.outputDirectory,
		outputNaming: getOutputNamingConfig(),
	};
}

/**
 * Updates output directory in state
 */
export function updateOutputDirectory(path: string): void {
	currentState.outputDirectory = path;
	publishOutputDraft({ directory: path });
}

/**
 * Updates naming preset in state.
 */
export function updateNamingPreset(preset: OutputNamingPreset): void {
	currentState.namingPreset = preset;
	publishOutputDraft({ namingPreset: preset });
}

/**
 * Backward-compatible mapping for existing callers.
 */
export function updateAbsCompatible(enabled: boolean): void {
	updateNamingPreset(enabled ? 'absDefault' : 'customTemplate');
}

/**
 * Updates naming template in state.
 */
export function updateNamingTemplate(template: string): void {
	currentState.namingTemplate = template;
	publishOutputDraft({ namingTemplate: template });
}

/**
 * Updates ABS year toggle in state
 */
export function updateAbsIncludeYear(enabled: boolean): void {
	currentState.absIncludeYear = enabled;
	publishOutputDraft({ includeYear: enabled });
}

/**
 * Updates encoder settings used by output sizing and processing payloads.
 */
export function updateEncoderSettings(settings: EncoderSettings): void {
	currentState.encoderSettings = settings;
}

/**
 * Updates sample rate used by processing payloads.
 */
export function updateSampleRate(sampleRate: SampleRateConfig): void {
	currentState.sampleRate = sampleRate;
}

/**
 * Loads initial state from DOM elements
 */
export function loadInitialState(): void {
	const namingPresetSelect = document.getElementById('output-naming-preset') as HTMLSelectElement;
	const namingTemplateInput = document.getElementById('output-template-input') as HTMLInputElement;
	const absYearCheckbox = document.getElementById('output-abs-include-year') as HTMLInputElement;

	if (
		namingPresetSelect?.value === 'absDefault' ||
		namingPresetSelect?.value === 'customTemplate'
	) {
		currentState.namingPreset = namingPresetSelect.value;
	}

	if (namingTemplateInput) {
		currentState.namingTemplate = namingTemplateInput.value;
	}

	if (absYearCheckbox) {
		currentState.absIncludeYear = absYearCheckbox.checked;
	}

	publishOutputDraft({
		directory: currentState.outputDirectory,
		namingPreset: currentState.namingPreset,
		namingTemplate: currentState.namingTemplate,
		includeYear: currentState.absIncludeYear,
	});
}
