/**
 * Output panel module - manages output directory, naming patterns, and encoder settings
 *
 * Re-exports public API from submodules.
 */
import { mount, unmount } from 'svelte';
import type { OutputConfig } from '../../types/audio';
import { toBoundaryEncoderSettings, type EncoderSettingsLike } from '../../types/encoder';
import OutputPanelIsland from './OutputPanelIsland.svelte';
import { getOutputNamingConfig, getState, loadInitialState } from './state';
import { setupEventHandlers } from './handlers';
import { updateOutputPath, updateEstimatedSize, updateNamingOptionState } from './dom';

const OUTPUT_PANEL_ROOT_ID = 'output-panel-root';

type WindowWithEncoderProvider = Window & {
	EncoderSettingsProvider?: () => EncoderSettingsLike;
};

let mountedOutputPanelRoot: HTMLElement | null = null;
let mountedOutputPanelIsland: Parameters<typeof unmount>[0] | null = null;

function mountOutputPanelIsland(): void {
	const outputPanelRoot = document.getElementById(OUTPUT_PANEL_ROOT_ID);
	if (!outputPanelRoot) return;

	if (
		mountedOutputPanelIsland &&
		mountedOutputPanelRoot === outputPanelRoot &&
		outputPanelRoot.childElementCount > 0
	) {
		return;
	}

	if (mountedOutputPanelIsland) {
		void unmount(mountedOutputPanelIsland);
		mountedOutputPanelIsland = null;
	}

	mountedOutputPanelIsland = mount(OutputPanelIsland, { target: outputPanelRoot });
	mountedOutputPanelRoot = outputPanelRoot;
}

/**
 * Initializes the output panel with event handlers
 */
export function initOutputPanel(): void {
	mountOutputPanelIsland();
	setupEventHandlers();
	loadInitialState();
	updateNamingOptionState();
	updateOutputPath();
	updateEstimatedSize();
}

/**
 * Gets current output configuration for processing
 */
export function getCurrentOutputConfig(): OutputConfig {
	const state = getState();
	if (!state.outputDirectory) {
		throw new Error('Output directory not selected');
	}

	// Return base directory; backend handles structure generation
	let encoderSettings = state.encoderSettings;
	const provider = (window as WindowWithEncoderProvider).EncoderSettingsProvider;
	if (provider) {
		const raw = provider();
		if (raw) {
			encoderSettings = toBoundaryEncoderSettings(raw);
		}
	}

	return {
		encoderSettings,
		sampleRate: state.sampleRate,
		outputPath: state.outputDirectory,
		outputNaming: getOutputNamingConfig(),
	};
}

/**
 * Updates output panel when file list changes
 */
export function onFileListChange(): void {
	updateEstimatedSize();
}

/**
 * Updates output panel when metadata changes
 */
export function onMetadataChange(): void {
	updateOutputPath();
	updateEstimatedSize();
}

// Re-export commonly needed items
export { getState } from './state';
export { updateOutputPath, updateEstimatedSize, getCurrentMetadata } from './dom';
export { sanitizeFilename, calculateOutputPath } from './pathBuilder';
