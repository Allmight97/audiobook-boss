/**
 * Output panel module - manages output directory, naming patterns, and encoder settings
 *
 * Re-exports public API from submodules.
 */
import { mount, unmount } from 'svelte';
import OutputPanelIsland from './OutputPanelIsland.svelte';
import { loadInitialState } from './state';
import { setupEventHandlers } from './handlers';
import { updateOutputPath, updateEstimatedSize, updateNamingOptionState } from './dom';

const OUTPUT_PANEL_ROOT_ID = 'output-panel-root';

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

// Re-export commonly needed items
export { getState } from './state';
export { readOutputConfigForProcessing } from './state';
export { updateOutputPath, updateEstimatedSize, getCurrentMetadata } from './dom';
export { sanitizeFilename, calculateOutputPath } from './pathBuilder';
