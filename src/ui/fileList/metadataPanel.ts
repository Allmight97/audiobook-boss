import { pathBasename } from '../../lib/path/basename';
import { type AudioFile, formatFileSize } from '../../types/audio';
import { tauriClient } from '../../lib/tauri/client';
import type { AudiobookMetadata } from '../../types/metadata';
import { updateEstimatedSize, updateOutputPath } from '../outputPanel';
import { refreshMetadataArtifacts } from '../metadataArtifacts';
import { updateTagPreview } from '../tagPreview';
import { clearCoverArt, getHasCustomCoverArt, refreshCoverArtDisplay } from '../coverArt';
import { effectiveCoverForFile } from '../coverArt/coverOwner';
import { getJobType } from '../jobControls';
import {
	getMetadataForFile,
	getMetadataIntentPatchForFile,
	isUsableMetadataCache,
	setMetadataForFile,
} from '../metadataState';
import {
	populateMetadataFormMulti,
	populateMetadataFormSingle,
	resetDirtyState,
} from '../metadataForm';
import {
	renderAutoResolutionHints,
	resetAutoResolutionHints,
} from '../encoderPanel/autoResolutionHints';
import {
	setInspectorCompanions,
	resetInspectorState,
	setInspectorContext,
	setInspectorValues,
} from './inspectorState.svelte';
import { companionSummaryForInputIds } from '../remoteSource';
import {
	getCurrentFileList,
	getSelectedFileIndices,
	getSelectedFileIndex,
	getSelectedFiles,
} from './state.svelte';

let latestSingleSelectionRequestId = 0;
let latestAutoCoverRequestId = 0;

function refreshOutputForMetadataChange(): void {
	updateOutputPath('final');
	updateEstimatedSize();
}

function updatePropertiesContextSingle(file: AudioFile, index: number): void {
	const fileList = getCurrentFileList();
	if (!fileList) return;

	if (index < 0 || index >= fileList.files.length) {
		setInspectorContext({ text: 'No file selected', variant: 'empty' });
		return;
	}

	const fileName = pathBasename(file.path, { fallback: 'path' });
	setInspectorContext({
		text: fileName,
		variant: 'single',
		detail: `${index + 1} of ${fileList.files.length}`,
	});
}

function updatePropertiesContextMulti(selectedCount: number): void {
	setInspectorContext({
		text: `${selectedCount} files selected`,
		variant: 'multi',
	});
}

function clearPropertyValues(): void {
	setInspectorValues({
		bitrateText: '---',
		sampleRateText: '---',
		channelsText: '---',
		codecText: '---',
		decoderText: '---',
		fileSizeText: '---',
	});
	setInspectorCompanions({ text: '---', title: '' });
}

function formatOptionalText(value: string | undefined): string {
	if (typeof value !== 'string') return 'N/A';
	const trimmed = value.trim();
	return trimmed.length > 0 ? trimmed : 'N/A';
}

function summarizeSharedTextValue(
	files: AudioFile[],
	pickValue: (file: AudioFile) => string | undefined,
): string {
	if (files.length === 0) return '---';

	const values = files.map((file) => {
		const value = pickValue(file);
		return typeof value === 'string' && value.trim().length > 0 ? value.trim() : null;
	});

	if (values.some((value) => value === null)) {
		return 'Mixed';
	}

	const unique = new Set(values);
	if (unique.size !== 1) {
		return 'Mixed';
	}

	return values[0] ?? 'Mixed';
}

async function loadMetadataForFile(file: AudioFile): Promise<Partial<AudiobookMetadata> | null> {
	if (!file.isValid) return null;

	const existing = getMetadataForFile(file.path);
	if (isUsableMetadataCache(existing)) return existing;

	try {
		const metadata = await tauriClient.readAudioMetadata(file.path);
		setMetadataForFile(file.path, metadata);
		return metadata;
	} catch (error) {
		console.warn('Failed to load metadata:', error);
		return null;
	}
}

export async function ensureMetadataForFiles(files: AudioFile[]): Promise<void> {
	const validFiles = files.filter((file) => file.isValid);
	await Promise.all(validFiles.map((file) => loadMetadataForFile(file)));
}

export function updateFileProperties(file: AudioFile): void {
	const companionSummary = companionSummaryForInputIds([file.inputId]);
	setInspectorCompanions({
		text: companionSummary.text,
		title: companionSummary.title,
	});

	if (file.isValid) {
		setInspectorValues({
			bitrateText: file.bitrate ? `${file.bitrate} kb/s` : 'N/A',
			sampleRateText: file.sampleRate ? `${file.sampleRate} Hz` : 'N/A',
			channelsText: file.channels ? `${file.channels} ch` : 'N/A',
			codecText: formatOptionalText(file.codecLabel),
			decoderText: formatOptionalText(file.selectedDecoder),
			fileSizeText: file.size ? formatFileSize(file.size) : 'N/A',
		});
	} else {
		clearPropertyValues();
	}

	updatePropertiesContextSingle(file, getSelectedFileIndex());
}

function updateMultiSelectionProperties(selectedFiles: AudioFile[]): void {
	updatePropertiesContextMulti(selectedFiles.length);
	const companionSummary = companionSummaryForInputIds(selectedFiles.map((file) => file.inputId));
	setInspectorCompanions({
		text: companionSummary.text,
		title: companionSummary.title,
	});
	setInspectorValues({
		bitrateText: '---',
		sampleRateText: '---',
		channelsText: '---',
		codecText: summarizeSharedTextValue(selectedFiles, (file) => file.codecLabel),
		decoderText: summarizeSharedTextValue(selectedFiles, (file) => file.selectedDecoder),
		fileSizeText: '---',
	});
}

function isCurrentSingleSelectionRequest(requestId: number, filePath: string): boolean {
	if (requestId !== latestSingleSelectionRequestId) return false;
	if (getSelectedFileIndices().size !== 1) return false;

	const fileList = getCurrentFileList();
	const selectedIndex = getSelectedFileIndex();
	if (!fileList || selectedIndex < 0 || selectedIndex >= fileList.files.length) {
		return false;
	}

	return fileList.files[selectedIndex]?.path === filePath;
}

function isCurrentMultiSelectionRequest(requestId: number, filePaths: string[]): boolean {
	if (requestId !== latestSingleSelectionRequestId) return false;

	const selectedIndices = Array.from(getSelectedFileIndices()).sort((a, b) => a - b);
	if (selectedIndices.length !== filePaths.length) return false;

	const fileList = getCurrentFileList();
	if (!fileList) return false;

	const selectedPaths = selectedIndices
		.map((index) => fileList.files[index]?.path)
		.filter((path): path is string => typeof path === 'string')
		.sort();
	const expectedPaths = [...filePaths].sort();

	if (selectedPaths.length !== expectedPaths.length) return false;
	return selectedPaths.every((path, index) => path === expectedPaths[index]);
}

function renderWorkbenchAutoResolutionHints(): void {
	const validFiles = getCurrentFileList()?.files.filter((file) => file.isValid) ?? [];
	if (validFiles.length > 0) {
		renderAutoResolutionHints(validFiles);
		return;
	}

	resetAutoResolutionHints();
}

export async function showSingleSelection(file: AudioFile): Promise<void> {
	const requestId = ++latestSingleSelectionRequestId;
	renderAutoResolutionHints([file]);
	updateFileProperties(file);
	refreshOutputForMetadataChange();
	updateTagPreview();
	refreshMetadataArtifacts();

	const stored = getMetadataForFile(file.path);
	if (isUsableMetadataCache(stored)) {
		if (!isCurrentSingleSelectionRequest(requestId, file.path)) {
			return;
		}
		populateMetadataFormSingle(stored);
		refreshOutputForMetadataChange();
		updateTagPreview();
		refreshMetadataArtifacts();
	} else {
		const metadata = await loadMetadataForFile(file);
		if (!metadata || !isCurrentSingleSelectionRequest(requestId, file.path)) {
			return;
		}

		populateMetadataFormSingle(metadata);
		refreshOutputForMetadataChange();
		updateTagPreview();
		refreshMetadataArtifacts();
	}
}

export async function showMultiSelection(selectedFiles: AudioFile[]): Promise<void> {
	const requestId = ++latestSingleSelectionRequestId;
	renderAutoResolutionHints(selectedFiles);
	updateMultiSelectionProperties(selectedFiles);

	resetDirtyState();

	const validFiles = selectedFiles.filter((file) => file.isValid);
	const metadataList = await Promise.all(
		validFiles.map(async (file) => {
			const metadata = await loadMetadataForFile(file);
			return metadata ?? {};
		}),
	);

	if (
		!isCurrentMultiSelectionRequest(
			requestId,
			selectedFiles.map((file) => file.path),
		)
	) {
		return;
	}

	populateMetadataFormMulti(metadataList, selectedFiles.length);
	refreshOutputForMetadataChange();
	updateTagPreview();
	refreshMetadataArtifacts();
	refreshCoverArtDisplay();
}

export function refreshSelectionPresentation(selectedFiles: AudioFile[]): void {
	latestSingleSelectionRequestId += 1;
	renderAutoResolutionHints(selectedFiles);

	if (selectedFiles.length === 1 && selectedFiles[0]) {
		updateFileProperties(selectedFiles[0]);
	} else if (selectedFiles.length > 1) {
		updateMultiSelectionProperties(selectedFiles);
	} else {
		clearSelectionPanels();
		return;
	}

	refreshOutputForMetadataChange();
	updateTagPreview();
}

export function clearSelectionPanels(): void {
	renderWorkbenchAutoResolutionHints();
	resetInspectorState();
	populateMetadataFormSingle({});
	clearCoverArt();
}

export async function autoUpdateCoverArtFromFirstValidFile(): Promise<void> {
	const requestId = ++latestAutoCoverRequestId;
	try {
		const fileList = getCurrentFileList();
		if (!fileList?.files.length) {
			refreshCoverArtDisplay();
			return;
		}

		const jobType = getJobType();
		const selectedValid = getSelectedFiles().find((file) => file.isValid);
		const firstValid = fileList.files.find((file) => file.isValid);
		const targetPath =
			jobType === 'merge' ? firstValid?.path : (selectedValid?.path ?? firstValid?.path);
		if (!targetPath) {
			refreshCoverArtDisplay();
			return;
		}

		if (
			effectiveCoverForFile(targetPath) !== null ||
			getMetadataIntentPatchForFile(targetPath)?.cover_art
		) {
			if (requestId === latestAutoCoverRequestId) {
				refreshCoverArtDisplay();
			}
			return;
		}

		const metadata = await tauriClient.readAudioMetadata(targetPath);
		const currentJobType = getJobType();
		const currentFileList = getCurrentFileList();
		const currentFirstValid = currentFileList?.files.find((file) => file.isValid);
		const currentSelectedValid = getSelectedFiles().find((file) => file.isValid);
		const currentTargetPath =
			currentJobType === 'merge'
				? currentFirstValid?.path
				: (currentSelectedValid?.path ?? currentFirstValid?.path);
		if (
			requestId !== latestAutoCoverRequestId ||
			getHasCustomCoverArt() ||
			getMetadataIntentPatchForFile(targetPath)?.cover_art ||
			currentJobType !== jobType ||
			currentTargetPath !== targetPath
		) {
			return;
		}

		const existing = getMetadataForFile(targetPath) ?? {};
		setMetadataForFile(targetPath, {
			...metadata,
			...existing,
			cover_art: metadata.cover_art || existing.cover_art,
		});
		refreshCoverArtDisplay();
	} catch (error) {
		if (requestId !== latestAutoCoverRequestId) {
			return;
		}
		refreshCoverArtDisplay();
		console.warn('Failed to auto-load cover art:', error);
	}
}

export { getSelectedFiles } from './state.svelte';
