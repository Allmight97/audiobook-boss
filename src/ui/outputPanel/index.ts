/**
 * Output panel module - manages output directory, naming patterns, and encoder settings
 *
 * Re-exports public API from submodules.
 */
import { loadInitialState } from './state';
import { setupEventHandlers } from './handlers';
import { updateOutputPath, updateEstimatedSize, updateNamingOptionState } from './dom';

/**
 * Initializes the output panel with event handlers
 */
export function initOutputPanel(): void {
	setupEventHandlers();
	loadInitialState();
	updateNamingOptionState();
	updateOutputPath();
	updateEstimatedSize();
}

// Re-export commonly needed items
export { getState } from './state';
export { readOutputConfigForProcessing } from './state';
export { updateOutputPath, updateEstimatedSize, getCurrentMetadata } from './dom';
export { sanitizeFilename, calculateOutputPath } from './pathBuilder';
export { default as OutputPanelIsland } from './OutputPanelIsland.svelte';
