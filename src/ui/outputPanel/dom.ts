/**
 * DOM manipulation for output panel
 */
import type { AudiobookMetadata } from '../../types/metadata';
import { formatFileSize } from '../../types/audio';
import { getCurrentFileList } from '../fileList';
import { getCurrentCoverArt } from '../coverArt';
import { getState } from './state';
import {
	getSeriesPartValidationError,
	getSubseriesPartValidationError,
} from '../metadataValidation';
import { calculateOutputPath } from './pathBuilder';

/**
 * Gets current metadata from the metadata panel DOM elements
 */
export function getCurrentMetadata(): AudiobookMetadata {
	const getElementValue = (id: string): string => {
		const element = document.getElementById(id) as HTMLInputElement;
		return element?.value || '';
	};

	const coverArt = getCurrentCoverArt();
	const title = getElementValue('meta-title');

	return {
		title: title,
		artist: getElementValue('meta-author'),
		album: title,
		composer: getElementValue('meta-narrator'),
		date: parseInt(getElementValue('meta-year'), 10) || undefined,
		genre: getElementValue('meta-genre'),
		description: getElementValue('meta-description'),
		series: getElementValue('meta-series'),
		series_part: getElementValue('meta-series-part') || undefined,
		subseries: getElementValue('meta-subseries'),
		subseries_part: getElementValue('meta-subseries-part') || undefined,
		cover_art: coverArt ?? undefined,
	};
}

/**
 * Updates the series part warning based on current metadata
 */
export function updateSeriesPartWarning(metadata: AudiobookMetadata): void {
	const warning = document.getElementById('meta-series-part-warning');
	if (!warning) return;

	const seriesValue = metadata.series?.trim() ?? '';
	const seriesPartValue = metadata.series_part?.trim() ?? '';
	const subseriesValue = metadata.subseries?.trim() ?? '';
	const subseriesPartValue = metadata.subseries_part?.trim() ?? '';
	const seriesPartError = getSeriesPartValidationError(seriesPartValue);

	if (seriesPartError) {
		warning.textContent = seriesPartError;
		warning.toggleAttribute('hidden', false);
		return;
	}

	const shouldShowDuplicate =
		seriesValue.length > 0 &&
		subseriesValue.length > 0 &&
		seriesPartValue.length > 0 &&
		subseriesPartValue.length > 0 &&
		seriesPartValue === subseriesPartValue;

	if (shouldShowDuplicate) {
		warning.textContent =
			'Book # matches sub-series #. Keep them aligned only when both series use the same sequence.';
		warning.toggleAttribute('hidden', false);
		return;
	}

	const shouldShow = seriesValue.length > 0 && seriesPartValue.length === 0;
	warning.textContent = 'Series detected - add Book # (series sequence) for ABS ordering.';
	warning.toggleAttribute('hidden', !shouldShow);
}

export function updateSubseriesPartWarning(metadata: AudiobookMetadata): void {
	const warning = document.getElementById('meta-subseries-part-warning');
	if (!warning) return;

	const subseriesValue = metadata.subseries?.trim() ?? '';
	const subseriesPartValue = metadata.subseries_part?.trim() ?? '';
	const subseriesPartError = getSubseriesPartValidationError(subseriesPartValue);

	if (subseriesPartError) {
		warning.textContent = subseriesPartError;
		warning.toggleAttribute('hidden', false);
		return;
	}

	const shouldShow = subseriesValue.length > 0 && subseriesPartValue.length === 0;
	warning.textContent =
		'Sub-series detected - add sub-series # (series sequence) for ABS ordering.';
	warning.toggleAttribute('hidden', !shouldShow);
}

/**
 * Updates the output path PREVIEW display
 */
export function updateOutputPath(): void {
	const state = getState();
	const previewText = document.getElementById('output-preview-text');
	const outputPathInput = document.getElementById('output-dir-text') as HTMLInputElement;

	const metadata = getCurrentMetadata();
	updateSeriesPartWarning(metadata);
	updateSubseriesPartWarning(metadata);

	// Basic validation state
	if (!state.outputDirectory) {
		if (previewText) previewText.textContent = 'Select output directory...';
		if (previewText) previewText.title = 'No directory selected';
		if (outputPathInput) outputPathInput.value = '';
		return;
	}

	// Update hidden input for reference
	if (outputPathInput) outputPathInput.value = state.outputDirectory;

	// Calculate generic preview (using First File or Placeholder)
	const calculatedPath = calculateOutputPath(metadata);

	if (previewText) {
		previewText.textContent = calculatedPath;
		previewText.title = calculatedPath; // Tooltip for full path
	}
}

/**
 * Updates naming hints based on ABS toggle
 */
export function updateNamingOptionState(): void {
	const state = getState();
	const absHint = document.getElementById('output-abs-hint');

	if (absHint) {
		absHint.toggleAttribute('hidden', !state.absCompatible);
		if (state.absCompatible) {
			absHint.textContent = state.absIncludeYear
				? 'Creates Author / Series / (Sub-series) / Book # - YYYY - Title'
				: 'Creates Author / Series / (Sub-series) / Book # - Title';
		}
	}
}

/**
 * Calculates estimated output file size in bytes
 */
function calculateEstimatedSize(totalDurationSeconds: number): number {
	const state = getState();
	if (!totalDurationSeconds || totalDurationSeconds <= 0) {
		return 0;
	}

	const encoderSettings = state.encoderSettings;

	// Base calculation: duration * bitrate / 8 (convert bits to bytes)
	let sizeBytes = (totalDurationSeconds * encoderSettings.bitrateKbps * 1000) / 8;

	// Adjust for stereo (roughly 1.5x mono at same bitrate)
	if (encoderSettings.channels === 'stereo') {
		sizeBytes *= 1.5;
	}

	// Add M4B container overhead (approximately 3%)
	sizeBytes *= 1.03;

	return Math.round(sizeBytes);
}

/**
 * Updates the estimated output size display
 */
export function updateEstimatedSize(): void {
	const sizeElement = document.getElementById('estimated-size');
	if (!sizeElement) return;

	const fileList = getCurrentFileList();
	if (!fileList || !fileList.files.length) {
		sizeElement.textContent = '~ --- MB';
		return;
	}

	const estimatedBytes = calculateEstimatedSize(fileList.totalDuration);
	sizeElement.textContent = `~ ${formatFileSize(estimatedBytes)}`;
}

/**
 * Shows an error message in the output panel
 */
export function showOutputError(message: string): void {
	console.error('Output Panel Error:', message);
}
