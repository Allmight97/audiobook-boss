import { formatFileSize } from '../../types/audio';
import type { AudiobookMetadata } from '../../types/metadata';
import { tauriClient } from '../../lib/tauri/client';
import { getCurrentFileList, getSelectedFileIndices } from '../fileList';
import { getCurrentCoverArt } from '../coverArt';
import { readMetadataForm } from '../metadataForm';
import {
	setSeriesPartWarning,
	setSubseriesPartWarning,
	metadataFormState,
} from '../metadataForm/state.svelte';
import {
	beginOutputPreviewRequest,
	getState,
	setEstimatedSizeText,
	setOutputNamingUiState,
} from './state.svelte';
import { readEncodingRequestConfig } from '../encoderPanel';
import { validateMetadataDraft } from '../metadataSession';
import type { OutputKind } from '../../types/audio';
import { runOutputPathPreviewWorkflow } from './outputPlanWorkflow';

export type OutputPathPreviewMetadataDraft = AudiobookMetadata;

type OutputPathPreviewContext = {
	outputDirectory: string;
	sourcePath?: string;
};

export function readOutputPathPreviewMetadataDraft(): OutputPathPreviewMetadataDraft {
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

function updateSeriesPartWarning(
	metadata: AudiobookMetadata,
	seriesPartError: string | null,
): void {
	const seriesValue = metadata.series?.trim() ?? '';
	const seriesPartValue = metadata.series_part?.trim() ?? '';
	const subseriesValue = metadata.subseries?.trim() ?? '';
	const subseriesPartValue = metadata.subseries_part?.trim() ?? '';

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

function updateSubseriesPartWarning(
	metadata: AudiobookMetadata,
	subseriesPartError: string | null,
): void {
	const subseriesValue = metadata.subseries?.trim() ?? '';
	const subseriesPartValue = metadata.subseries_part?.trim() ?? '';

	if (subseriesPartError) {
		setSubseriesPartWarning(subseriesPartError, true);
		return;
	}

	const message = 'Sub-series detected - add sub-series # (series sequence) for ABS ordering.';
	const visible = subseriesValue.length > 0 && subseriesPartValue.length === 0;
	setSubseriesPartWarning(message, visible);
}

export async function updateMetadataIntentWarnings(metadata: AudiobookMetadata): Promise<void> {
	const validation = await validateMetadataDraft(metadata, tauriClient.validateMetadataIntentPatch);
	updateSeriesPartWarning(metadata, validation.errors.byField.series_part ?? null);
	updateSubseriesPartWarning(metadata, validation.errors.byField.subseries_part ?? null);
}

export function updateOutputPath(outputKind: OutputKind): void {
	const requestId = getState().outputDirectory ? beginOutputPreviewRequest() : undefined;
	void runOutputPathPreviewWorkflow(outputKind, undefined, requestId);
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

export function buildOutputPathPreviewContext(): OutputPathPreviewContext {
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
	if (!totalDurationSeconds || totalDurationSeconds <= 0) {
		return 0;
	}

	const { encoderSettings } = readEncodingRequestConfig();
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
