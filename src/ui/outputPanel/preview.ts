import { formatFileSize } from '../../types/audio';
import type { AudiobookMetadata } from '../../types/metadata';
import { tauriClient } from '../../lib/tauri/client';
import { getCurrentFileList } from '../fileList';
import { getSelectedFileIndices } from '../fileList/state';
import { getCurrentCoverArt } from '../coverArt';
import { readMetadataForm } from '../metadataForm';
import {
	setSeriesPartWarning,
	setSubseriesPartWarning,
	metadataFormState,
} from '../metadataForm/state.svelte';
import {
	beginOutputPreviewRequest,
	isLatestOutputPreviewRequest,
	getOutputNamingConfig,
	getState,
	setEstimatedSizeText,
	setOutputNamingUiState,
	setOutputPreview,
} from './state.svelte';
import {
	getSeriesPartValidationError,
	getSubseriesPartValidationError,
} from '../metadataValidation';
import type { OutputKind } from '../../types/audio';

type OutputPreviewCallSiteState = {
	outputDirectory: string;
	sourcePath?: string;
};

export function getCurrentMetadata(): AudiobookMetadata {
	const metadata = readMetadataForm({
		mode: metadataFormState.mode,
		onlyDirty: false,
		includeCoverArt: true,
	});
	const coverArt = getCurrentCoverArt();

	return {
		title: metadata.title ?? '',
		album: metadata.album ?? metadata.title ?? '',
		artist: metadata.artist ?? '',
		composer: metadata.composer ?? '',
		date: metadata.date,
		genre: metadata.genre ?? '',
		description: metadata.description ?? '',
		series: metadata.series ?? '',
		series_part: metadata.series_part ?? undefined,
		subseries: metadata.subseries ?? '',
		subseries_part: metadata.subseries_part ?? undefined,
		cover_art: coverArt ?? metadata.cover_art ?? undefined,
	};
}

export function updateSeriesPartWarning(metadata: AudiobookMetadata): void {
	const seriesValue = metadata.series?.trim() ?? '';
	const seriesPartValue = metadata.series_part?.trim() ?? '';
	const subseriesValue = metadata.subseries?.trim() ?? '';
	const subseriesPartValue = metadata.subseries_part?.trim() ?? '';
	const seriesPartError = getSeriesPartValidationError(seriesPartValue);

	if (seriesPartError) {
		setSeriesPartWarning(seriesPartError, true);
		return;
	}

	const shouldShowDuplicate =
		seriesValue.length > 0 &&
		subseriesValue.length > 0 &&
		seriesPartValue.length > 0 &&
		subseriesPartValue.length > 0 &&
		seriesPartValue === subseriesPartValue;

	if (shouldShowDuplicate) {
		const message =
			'Book # matches sub-series #. Keep them aligned only when both series use the same sequence.';
		setSeriesPartWarning(message, true);
		return;
	}

	const message = 'Series detected - add Book # (series sequence) for ABS ordering.';
	const visible = seriesValue.length > 0 && seriesPartValue.length === 0;
	setSeriesPartWarning(message, visible);
}

export function updateSubseriesPartWarning(metadata: AudiobookMetadata): void {
	const subseriesValue = metadata.subseries?.trim() ?? '';
	const subseriesPartValue = metadata.subseries_part?.trim() ?? '';
	const subseriesPartError = getSubseriesPartValidationError(subseriesPartValue);

	if (subseriesPartError) {
		setSubseriesPartWarning(subseriesPartError, true);
		return;
	}

	const message = 'Sub-series detected - add sub-series # (series sequence) for ABS ordering.';
	const visible = subseriesValue.length > 0 && subseriesPartValue.length === 0;
	setSubseriesPartWarning(message, visible);
}

export function updateOutputPath(outputKind: OutputKind): void {
	void updateOutputPathAsync(outputKind);
}

async function updateOutputPathAsync(outputKind: OutputKind): Promise<void> {
	const state = getState();
	const metadata = getCurrentMetadata();

	updateSeriesPartWarning(metadata);
	updateSubseriesPartWarning(metadata);

	if (!state.outputDirectory) {
		setOutputPreview('Select output directory...', 'No directory selected');
		return;
	}

	const previewCallSiteState = buildOutputPreviewCallSiteState();
	const requestId = beginOutputPreviewRequest();
	try {
		const previewPath = await tauriClient.previewOutputPath({
			outputDir: previewCallSiteState.outputDirectory,
			metadata,
			outputNaming: getOutputNamingConfig(),
			sourcePath: previewCallSiteState.sourcePath,
			outputKind,
		});
		if (!isLatestOutputPreviewRequest(requestId)) {
			return;
		}
		setOutputPreview(previewPath);
	} catch (error) {
		if (!isLatestOutputPreviewRequest(requestId)) {
			return;
		}
		const message = 'Output preview unavailable. Fix metadata/template and retry.';
		setOutputPreview(message);
		showOutputError(`Rust preview failed: ${String(error)}`);
	}
}

export function updateNamingOptionState(): void {
	const state = getState();
	const absPresetEnabled = state.namingPreset === 'absDefault';
	setOutputNamingUiState({
		absHintHidden: !absPresetEnabled,
		absHintText: absPresetEnabled
			? state.absIncludeYear
				? 'Creates Author / Series / (Sub-series) / Book # - YYYY - Title'
				: 'Creates Author / Series / (Sub-series) / Book # - Title'
			: '',
		templateRowHidden: state.namingPreset !== 'customTemplate',
	});
}

export function buildOutputPreviewCallSiteState(): OutputPreviewCallSiteState {
	const state = getState();
	const fileList = getCurrentFileList();
	const selectedIndices = Array.from(getSelectedFileIndices()).sort((a, b) => a - b);
	const sourcePath =
		selectedIndices.length > 0
			? fileList?.files[selectedIndices[0]]?.path
			: fileList?.files.find((file) => file.isValid)?.path;
	return {
		outputDirectory: state.outputDirectory,
		sourcePath,
	};
}

function calculateEstimatedSize(totalDurationSeconds: number): number {
	const state = getState();
	if (!totalDurationSeconds || totalDurationSeconds <= 0) {
		return 0;
	}

	const encoderSettings = state.encoderSettings;
	let sizeBytes = (totalDurationSeconds * encoderSettings.bitrateKbps * 1000) / 8;

	if (encoderSettings.channels === 'stereo') {
		sizeBytes *= 1.5;
	}

	sizeBytes *= 1.03;
	return Math.round(sizeBytes);
}

export function updateEstimatedSize(): void {
	const fileList = getCurrentFileList();
	if (!fileList?.files.length) {
		setEstimatedSizeText('~ --- MB');
		return;
	}

	const estimatedBytes = calculateEstimatedSize(fileList.totalDuration);
	setEstimatedSizeText(`~ ${formatFileSize(estimatedBytes)}`);
}

export function showOutputError(message: string): void {
	console.error('Output Panel Error:', message);
}
