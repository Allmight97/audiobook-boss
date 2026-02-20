/**
 * Event handlers for output panel controls
 */
import type { EncoderSettings } from '../../types/audio';
import { tauriClient } from '../../lib/tauri/client';
import {
	updateEncoderSettings,
	updateSampleRate,
	updateChannels,
	updateOutputDirectory,
	updateAbsCompatible,
	updateAbsIncludeYear,
} from './state';
import {
	updateOutputPath,
	updateEstimatedSize,
	updateNamingOptionState,
	showOutputError,
} from './dom';

/**
 * Handles bitrate selection change
 */
export function handleBitrateChange(event: Event): void {
	const target = event.target as HTMLSelectElement;
	updateEncoderSettings({
		bitrateKbps: parseInt(target.value, 10) as EncoderSettings['bitrateKbps'],
	});
	updateEstimatedSize();
}

/**
 * Handles sample rate selection change
 */
export function handleSampleRateChange(event: Event): void {
	const target = event.target as HTMLSelectElement;
	updateSampleRate(target.value);
	updateEstimatedSize();
}

/**
 * Handles channel configuration change
 */
export function handleChannelsChange(event: Event): void {
	const target = event.target as HTMLSelectElement;
	updateChannels(target.value);
	updateEstimatedSize();
}

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
 * Sets up audio settings event handlers
 */
export function setupSettingsHandlers(): void {
	const bitrateSelect = document.getElementById('output-bitrate') as HTMLSelectElement;
	const sampleRateSelect = document.getElementById('output-samplerate') as HTMLSelectElement;
	const channelsSelect = document.getElementById('output-channels') as HTMLSelectElement;

	if (bitrateSelect) {
		bitrateSelect.addEventListener('change', handleBitrateChange);
	}

	if (sampleRateSelect) {
		sampleRateSelect.addEventListener('change', handleSampleRateChange);
	}

	if (channelsSelect) {
		channelsSelect.addEventListener('change', handleChannelsChange);
	}
}

/**
 * Sets up all event handlers for output settings controls
 */
export function setupEventHandlers(): void {
	setupSettingsHandlers();

	document.addEventListener('abb:job-type-changed', () => {
		updateOutputPath();
	});
}
