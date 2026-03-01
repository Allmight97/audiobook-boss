import { type AudioFile, formatFileSize } from '../../types/audio';
import { tauriClient } from '../../lib/tauri/client';
import type { AudiobookMetadata } from '../../types/metadata';
import { updateEstimatedSize, updateOutputPath } from '../outputPanel';
import { updateTagPreview } from '../tagPreview';
import { clearCoverArt, getHasCustomCoverArt, setCoverArt } from '../coverArt';
import { getMetadataForFile, setMetadataForFile } from '../metadataState';
import {
	populateMetadataFormMulti,
	populateMetadataFormSingle,
	resetDirtyState,
} from '../metadataForm';
import {
	renderAutoResolutionHints,
	resetAutoResolutionHints,
} from '../encoderPanel/autoResolutionHints';
import { getCurrentFileList, getSelectedFileIndices, getSelectedFileIndex } from './state';

function refreshOutputForMetadataChange(): void {
	updateOutputPath();
	updateEstimatedSize();
}

function setText(id: string, value: string): void {
	const el = document.getElementById(id);
	if (el) el.textContent = value;
}

function updatePropertiesContextSingle(file: AudioFile, index: number): void {
	const contextEl = document.getElementById('prop-selected-context');
	const fileList = getCurrentFileList();
	if (!contextEl || !fileList) return;

	contextEl.replaceChildren();

	if (index < 0 || index >= fileList.files.length) {
		const emptySpan = document.createElement('span');
		emptySpan.className = 'context-empty';
		emptySpan.textContent = 'No file selected';
		contextEl.appendChild(emptySpan);
		return;
	}

	const fileName = file.path.split(/[\\/]/).pop() || file.path;
	const totalFiles = fileList.files.length;

	const filenameSpan = document.createElement('span');
	filenameSpan.className = 'context-filename';
	filenameSpan.title = fileName;
	filenameSpan.textContent = fileName;

	const posSpan = document.createElement('span');
	posSpan.className = 'context-position';
	posSpan.textContent = `${index + 1} of ${totalFiles}`;

	contextEl.appendChild(filenameSpan);
	contextEl.appendChild(posSpan);
}

function updatePropertiesContextMulti(selectedCount: number): void {
	const contextEl = document.getElementById('prop-selected-context');
	if (!contextEl) return;

	contextEl.replaceChildren();

	const span = document.createElement('span');
	span.className = 'context-filename';
	span.textContent = `${selectedCount} files selected`;
	contextEl.appendChild(span);
}

function clearPropertiesContext(): void {
	const contextEl = document.getElementById('prop-selected-context');
	if (!contextEl) return;

	contextEl.replaceChildren();
	const emptySpan = document.createElement('span');
	emptySpan.className = 'context-empty';
	emptySpan.textContent = 'No file selected';
	contextEl.appendChild(emptySpan);
}

function clearPropertyValues(): void {
	setText('prop-bitrate', '---');
	setText('prop-samplerate', '---');
	setText('prop-channels', '---');
	setText('prop-codec', '---');
	setText('prop-decoder', '---');
	setText('prop-filesize', '---');
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
	if (existing) return existing;

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

export function updateFileProperties(
	file: AudioFile,
	options?: { skipMetadataLoad?: boolean },
): void {
	if (file.isValid) {
		setText('prop-bitrate', file.bitrate ? `${file.bitrate} kb/s` : 'N/A');
		setText('prop-samplerate', file.sampleRate ? `${file.sampleRate} Hz` : 'N/A');
		setText('prop-channels', file.channels ? `${file.channels} ch` : 'N/A');
		setText('prop-codec', formatOptionalText(file.codecLabel));
		setText('prop-decoder', formatOptionalText(file.selectedDecoder));
		setText('prop-filesize', file.size ? formatFileSize(file.size) : 'N/A');

		if (!options?.skipMetadataLoad) {
			void loadMetadataForFile(file).then((metadata) => {
				if (metadata) {
					populateMetadataFormSingle(metadata);
					refreshOutputForMetadataChange();
					updateTagPreview();
				}
			});
		}
	} else {
		clearPropertyValues();
	}

	updatePropertiesContextSingle(file, getSelectedFileIndex());
}

export async function showSingleSelection(file: AudioFile): Promise<void> {
	renderAutoResolutionHints([file]);

	const stored = getMetadataForFile(file.path);
	if (stored) {
		updateFileProperties(file, { skipMetadataLoad: true });
		populateMetadataFormSingle(stored);
	} else {
		updateFileProperties(file);
	}

	refreshOutputForMetadataChange();
	updateTagPreview();
}

export async function showMultiSelection(selectedFiles: AudioFile[]): Promise<void> {
	renderAutoResolutionHints(selectedFiles);

	const selectedCount = selectedFiles.length;

	updatePropertiesContextMulti(selectedCount);
	clearPropertyValues();
	setText(
		'prop-codec',
		summarizeSharedTextValue(selectedFiles, (file) => file.codecLabel),
	);
	setText(
		'prop-decoder',
		summarizeSharedTextValue(selectedFiles, (file) => file.selectedDecoder),
	);

	resetDirtyState();

	const validFiles = selectedFiles.filter((file) => file.isValid);
	const metadataList = await Promise.all(
		validFiles.map(async (file) => {
			const metadata = await loadMetadataForFile(file);
			return metadata ?? {};
		}),
	);

	populateMetadataFormMulti(metadataList, selectedCount);
	refreshOutputForMetadataChange();
	updateTagPreview();
}

export function clearSelectionPanels(): void {
	resetAutoResolutionHints();

	clearPropertyValues();
	clearPropertiesContext();
	populateMetadataFormSingle({});
	clearCoverArt();
}

export async function autoUpdateCoverArtFromFirstValidFile(): Promise<void> {
	try {
		if (getHasCustomCoverArt()) return;
		const fileList = getCurrentFileList();
		if (!fileList || !fileList.files.length) {
			setCoverArt(null);
			return;
		}
		const firstValid = fileList.files.find((f) => f.isValid);
		if (!firstValid) {
			setCoverArt(null);
			return;
		}
		const metadata = await tauriClient.readAudioMetadata(firstValid.path);
		setCoverArt(metadata.cover_art || null);
	} catch (error) {
		setCoverArt(null);
		console.warn('Failed to auto-load cover art:', error);
	}
}

export function getSelectedFiles(): AudioFile[] {
	const fileList = getCurrentFileList();
	if (!fileList) return [];
	const selectedIndices = getSelectedFileIndices();
	return Array.from(selectedIndices)
		.map((index) => fileList.files[index])
		.filter((file): file is AudioFile => Boolean(file));
}
