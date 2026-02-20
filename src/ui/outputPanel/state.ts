/**
 * Output panel state management
 */
import type { EncoderSettings, SampleRateConfig, OutputNamingConfig } from '../../types/audio';
import { defaultEncoderSettings } from '../../types/audio';

/**
 * State interface for output panel configuration
 */
export interface OutputPanelState {
	encoderSettings: EncoderSettings;
	sampleRate: SampleRateConfig;
	outputDirectory: string;
	absCompatible: boolean;
	absIncludeYear: boolean;
}

/**
 * Current output panel state (module-level singleton)
 */
const currentState: OutputPanelState = {
	encoderSettings: { ...defaultEncoderSettings() },
	sampleRate: { explicit: 22050 },
	outputDirectory: '',
	absCompatible: true,
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
	return {
		absCompatible: currentState.absCompatible,
		includeYear: currentState.absIncludeYear,
	};
}

/**
 * Updates output directory in state
 */
export function updateOutputDirectory(path: string): void {
	currentState.outputDirectory = path;
}

/**
 * Updates ABS-compatible toggle in state
 */
export function updateAbsCompatible(enabled: boolean): void {
	currentState.absCompatible = enabled;
}

/**
 * Updates ABS year toggle in state
 */
export function updateAbsIncludeYear(enabled: boolean): void {
	currentState.absIncludeYear = enabled;
}

/**
 * Loads initial state from DOM elements
 */
export function loadInitialState(): void {
	const absStructureCheckbox = document.getElementById('output-abs-structure') as HTMLInputElement;
	const absYearCheckbox = document.getElementById('output-abs-include-year') as HTMLInputElement;

	if (absStructureCheckbox) {
		currentState.absCompatible = absStructureCheckbox.checked;
	}

	if (absYearCheckbox) {
		currentState.absIncludeYear = absYearCheckbox.checked;
	}
}
