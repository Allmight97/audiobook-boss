/**
 * Output panel module - manages output directory, naming patterns, and encoder settings
 *
 * Re-exports public API from submodules.
 */
import { loadInitialState } from './state.svelte';
import { resetOutputPanelHandlers } from './handlers';
import { updateOutputPath, updateEstimatedSize, updateNamingOptionState } from './dom';

/**
 * Initializes the output panel with event handlers
 */
export function initOutputPanel(): void {
	resetOutputPanelHandlers();
	loadInitialState();
	updateNamingOptionState();
	updateOutputPath();
	updateEstimatedSize();
}

// Re-export commonly needed items
export { getState } from './state.svelte';
export { readOutputConfigForProcessing } from './state.svelte';
export { updateOutputPath, updateEstimatedSize, getCurrentMetadata } from './dom';
export { sanitizeFilename, calculateOutputPath } from './pathBuilder';
export { default as OutputPanelIsland } from './OutputPanelIsland.svelte';
