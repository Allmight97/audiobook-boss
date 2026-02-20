/**
 * Event handlers for output panel controls
 */
import { tauriClient } from '../../lib/tauri/client';
import { updateOutputDirectory, updateAbsCompatible, updateAbsIncludeYear } from './state';
import { updateOutputPath, updateEstimatedSize, updateNamingOptionState, showOutputError } from './dom';

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
			updateOutputPath();
		}
	} catch (error) {
		console.error('Error selecting directory:', error);
		showOutputError('Failed to select directory');
	}
}

/**
 * Handles ABS-compatible structure checkbox change
 */
export function handleAbsCompatibleChange(event: Event): void {
	const target = event.target as HTMLInputElement;
	updateAbsCompatible(target.checked);
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
 * Sets up runtime event handlers for derived output updates.
 */
export function setupEventHandlers(): void {
	document.addEventListener('abb:job-type-changed', () => {
		updateOutputPath();
	});

	document.addEventListener('abb:encoder-settings-changed', () => {
		updateEstimatedSize();
	});
}
