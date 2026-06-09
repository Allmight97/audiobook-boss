/**
 * Output panel module - manages output directory, naming patterns, and output preview state.
 */
import type { OutputDefaults } from '../../types/appSettings';
import { applyOutputDefaults, loadInitialState } from './state.svelte';
import { resetOutputPanelActions } from './actions';
import { updateOutputPath, updateEstimatedSize, updateNamingOptionState } from './preview';

export function initOutputPanel(): void {
	resetOutputPanelActions();
	loadInitialState();
	updateNamingOptionState();
	updateOutputPath('final');
	updateEstimatedSize();
}

export function applyOutputDefaultsFromSettings(defaults: OutputDefaults): void {
	applyOutputDefaults(defaults);
	updateNamingOptionState();
	updateOutputPath('final');
	updateEstimatedSize();
}

export {
	getState,
	outputPanelState,
	readOutputDefaultsFromState,
	readOutputRequestConfig,
} from './state.svelte';
export { updateOutputPath, updateEstimatedSize, getCurrentMetadata } from './preview';
export { default as OutputPanelIsland } from './OutputPanelIsland.svelte';
