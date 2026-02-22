/**
 * Event handlers for output panel controls
 */
import { tauriClient } from '../../lib/tauri/client';
import { publishOutputDraft } from '../core/appStore.svelte';
import {
	updateOutputDirectory,
	updateNamingPreset,
	updateNamingTemplate,
	updateAbsIncludeYear,
} from './state';
import { updateOutputPath, updateNamingOptionState, showOutputError } from './dom';

/**
 * Handles directory browse button click
 */
export async function handleDirectoryBrowse(): Promise<void> {
	try {
		const selectedPath = await tauriClient.open({
			directory: true,
			multiple: false,
			title: 'Select Output Directory',
		});

		const normalized =
			typeof selectedPath === 'string'
				? selectedPath
				: Array.isArray(selectedPath) && selectedPath.length > 0
					? selectedPath[0]
					: null;

		if (normalized) {
			updateOutputDirectory(normalized);
			publishOutputDraft({ directory: normalized });
			updateOutputPath();
		}
	} catch (error) {
		console.error('Error selecting directory:', error);
		showOutputError('Failed to select directory');
	}
}

/**
 * Handles naming preset selector changes
 */
export function handleNamingPresetChange(event: Event): void {
	const target = event.target as HTMLSelectElement;
	const preset = target.value === 'customTemplate' ? 'customTemplate' : 'absDefault';
	updateNamingPreset(preset);
	updateNamingOptionState();
	updateOutputPath();
}

/**
 * Handles ABS include year checkbox change
 */
export function handleAbsIncludeYearChange(event: Event): void {
	const target = event.target as HTMLInputElement;
	updateAbsIncludeYear(target.checked);
	updateNamingOptionState();
	updateOutputPath();
}

/**
 * Handles custom template input changes
 */
export function handleNamingTemplateInput(event: Event): void {
	const target = event.target as HTMLInputElement;
	updateNamingTemplate(target.value);
	updateOutputPath();
}

/**
 * Sets up runtime event handlers for derived output updates.
 */
export function setupEventHandlers(): void {
	document.addEventListener('abb:job-type-changed', () => {
		updateOutputPath();
	});
}
