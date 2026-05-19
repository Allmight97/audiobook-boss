/**
 * Output panel module - manages output directory, naming patterns, and output preview state.
 */
import { loadInitialState } from './state.svelte';
import { resetOutputPanelActions } from './actions';
import { updateOutputPath, updateEstimatedSize, updateNamingOptionState } from './preview';

export function initOutputPanel(): void {
	resetOutputPanelActions();
	loadInitialState();
	updateNamingOptionState();
	updateOutputPath('final');
	updateEstimatedSize();
}

export { getState } from './state.svelte';
export { readOutputRequestConfig } from './state.svelte';
export { updateOutputPath, updateEstimatedSize, getCurrentMetadata } from './preview';
export { default as OutputPanelIsland } from './OutputPanelIsland.svelte';
