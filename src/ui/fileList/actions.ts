import type { AudioFile, FileListInfo } from '../../types/audio';
import { updateEstimatedSize } from '../outputPanel';
import { pushStatusPanelTransientStatus } from '../statusPanel';
import { clearMetadataSession, removeMetadataForFile } from '../metadataSession';
import {
	fileListSessionState,
	getCurrentFileList,
	getSelectedFileIndex,
	getSelectedFileIndices,
	setCurrentFileList,
	setSelectedIndex,
	setSelectedFileIndices,
	getSortDirection,
	setSortDirection,
	isOrderLocked,
	setOrderLocked,
	getImportOrdinal,
	recordImportOrder,
	removeImportOrdinal,
	resetImportOrder,
} from './state.svelte';
import {
	applySelectionIntent as applySelectionIntentToState,
	reindexSelectionAfterMove,
	reindexSelectionAfterRemoval,
	swapSelectionIndices,
	type SelectionIntent,
} from './selection';
import {
	autoUpdateCoverArtFromFirstValidFile,
	clearSelectionPanels,
	getSelectedFiles,
	coordinateMetadataSurfaceSelectionTransition,
	coordinateMetadataSurfacePresentationRefresh,
	showMultiSelection,
	showSingleSelection,
} from './metadataPanel';
import {
	buildFileListAppendResult,
	normalizeFileListInfo,
	type FileListAppendResult,
} from './appendResult';
import { preserveMetadataDraftsBeforeSelectionChange } from './metadataStaging';
import { purgeRemoteSourceSessionsForInputIds } from '../remoteSource';
import { pathBasename } from '../../lib/path/basename';
import { removeFileListCoverThumbnail } from './coverThumbnails.svelte';

function refreshOutputForFileListChange(): void {
	updateEstimatedSize();
}

function setTransientStatusMessage(message: string, timeoutMs: number = 2000): void {
	pushStatusPanelTransientStatus(message, { ttlMs: timeoutMs });
}

function replaceCurrentFileListFiles(nextFiles: AudioFile[]): void {
	const fileList = getCurrentFileList();
	if (!fileList) {
		return;
	}

	const validCount = nextFiles.filter((file) => file.isValid).length;
	fileListSessionState.currentFileList = {
		...fileList,
		files: nextFiles,
		validCount,
		invalidCount: nextFiles.length - validCount,
	};
	recalculateTotals();
}

function selectSoleImportedFile(fileList: FileListInfo): void {
	if (fileList.files.length !== 1) return;
	if (!fileList.files[0]?.isValid) return;

	setSelectedIndex(0);
	setSelectedFileIndices([0]);
	void showSingleSelection(fileList.files[0]);
}

export function displayFileList(fileListInfo: FileListInfo): void {
	const previousInputIds = getCurrentFileList()?.files.map((file) => file.inputId) ?? [];
	const normalizedFileListInfo = normalizeFileListInfo(fileListInfo);

	clearMetadataSession();
	setCurrentFileList(normalizedFileListInfo);
	resetImportOrder(normalizedFileListInfo.files);
	clearSelectionPanels();

	refreshOutputForFileListChange();

	selectSoleImportedFile(normalizedFileListInfo);
	void autoUpdateCoverArtFromFirstValidFile();
	void purgeRemoteSourceSessionsForInputIds(previousInputIds);
}

export function appendFileList(
	fileListInfo: FileListInfo,
	options?: { existingFiles?: AudioFile[]; showDuplicateStatus?: boolean },
): FileListAppendResult {
	const currentFileList = getCurrentFileList();
	const existingFiles = options?.existingFiles ?? currentFileList?.files ?? [];
	const appendResult = buildFileListAppendResult(fileListInfo, { existingFiles, currentFileList });

	if (appendResult.outcome === 'replace') {
		displayFileList(fileListInfo);
		return appendResult;
	}
	if (appendResult.outcome === 'duplicateOnly') {
		if (options?.showDuplicateStatus ?? true) {
			setTransientStatusMessage('No new files added. All analyzed files were already in the list.');
		}
		return appendResult;
	}

	const selectedIndex = getSelectedFileIndex();
	const selectedIndices = getSelectedFileIndices();
	setCurrentFileList(appendResult.fileList);
	recordImportOrder(appendResult.fileList.files);
	setSelectedIndex(selectedIndex);
	setSelectedFileIndices(selectedIndices);

	refreshOutputForFileListChange();
	return appendResult;
}

export async function selectFile(
	index: number,
	modifiers?: { multi: boolean },
	options?: { skipPersistPrevious?: boolean },
): Promise<void> {
	return applySelectionIntent(
		modifiers?.multi ? { type: 'toggle', index } : { type: 'selectOnly', index },
		options,
	);
}

export async function applySelectionIntent(
	intent: SelectionIntent,
	options?: {
		skipPersistPrevious?: boolean;
		openMetadataSurface?: boolean;
		anchor?: HTMLElement | null;
	},
): Promise<void> {
	await coordinateMetadataSurfaceSelectionTransition(intent, applySelectionIntentToState, {
		skipPersistPrevious: options?.skipPersistPrevious,
		openAfterPopulate: options?.openMetadataSurface,
		anchor: options?.anchor,
	});
}

export async function selectAll(): Promise<void> {
	return applySelectionIntent({ type: 'selectAll' });
}

export async function clearSelectionAction(): Promise<void> {
	return applySelectionIntent({ type: 'clear' });
}

export async function removeFile(index: number): Promise<void> {
	if (isOrderLocked()) return;
	const fileList = getCurrentFileList();
	if (!fileList || index < 0 || index >= fileList.files.length) {
		return;
	}

	const removedFile = fileList.files[index];
	removeMetadataForFile(removedFile.path);
	removeImportOrdinal(removedFile.path);
	removeFileListCoverThumbnail(removedFile.path);
	void purgeRemoteSourceSessionsForInputIds([removedFile.inputId]);

	const nextFiles = [...fileList.files];
	nextFiles.splice(index, 1);
	replaceCurrentFileListFiles(nextFiles);

	reindexSelectionAfterRemoval(index);

	const remainingSelection = getSelectedFiles();
	if (remainingSelection.length === 0) {
		clearSelectionPanels();
	} else if (remainingSelection.length === 1) {
		void showSingleSelection(remainingSelection[0]);
	} else {
		void showMultiSelection(remainingSelection);
	}

	refreshOutputForFileListChange();
}

/** Removes the active selection after its current metadata draft has been staged. */
export async function removeSelectedFiles(): Promise<void> {
	if (isOrderLocked()) return;
	const selectedIndices = Array.from(getSelectedFileIndices()).sort((left, right) => right - left);
	if (selectedIndices.length === 0) return;

	if (
		!(await preserveMetadataDraftsBeforeSelectionChange({
			validationFailureMessage: 'Fix metadata validation errors before removing selected files.',
		}))
	) {
		return;
	}

	for (const index of selectedIndices) {
		await removeFile(index);
	}
}

export function recalculateTotals(): void {
	const fileList = getCurrentFileList();
	if (!fileList) return;

	const validFiles = fileList.files.filter((file) => file.isValid && file.duration && file.size);
	const totalDuration = validFiles.reduce((sum, file) => sum + (file.duration || 0), 0);
	const totalSize = validFiles.reduce((sum, file) => sum + (file.size || 0), 0);
	fileListSessionState.currentFileList = {
		...fileList,
		totalDuration,
		totalSize,
	};
}

export function moveFileUp(index: number): void {
	if (isOrderLocked()) return;
	const fileList = getCurrentFileList();
	if (!fileList || index <= 0 || index >= fileList.files.length) {
		return;
	}

	const nextFiles = [...fileList.files];
	const temp = nextFiles[index];
	nextFiles[index] = nextFiles[index - 1];
	nextFiles[index - 1] = temp;
	replaceCurrentFileListFiles(nextFiles);
	setSortDirection('none');

	swapSelectionIndices(index, index - 1);

	refreshOutputForFileListChange();

	void coordinateMetadataSurfacePresentationRefresh();
}

export function moveFileDown(index: number): void {
	if (isOrderLocked()) return;
	const fileList = getCurrentFileList();
	if (!fileList || index < 0 || index >= fileList.files.length - 1) {
		return;
	}

	const nextFiles = [...fileList.files];
	const temp = nextFiles[index];
	nextFiles[index] = nextFiles[index + 1];
	nextFiles[index + 1] = temp;
	replaceCurrentFileListFiles(nextFiles);
	setSortDirection('none');

	swapSelectionIndices(index, index + 1);

	refreshOutputForFileListChange();

	void coordinateMetadataSurfacePresentationRefresh();
}

export async function toggleFileSort(): Promise<void> {
	if (isOrderLocked()) return;
	const fileList = getCurrentFileList();
	if (!fileList || fileList.files.length <= 1) return;

	if (
		!(await preserveMetadataDraftsBeforeSelectionChange({
			validationFailureMessage: 'Fix metadata validation errors before sorting files.',
		}))
	) {
		return;
	}

	const selectedPaths = new Set(
		Array.from(getSelectedFileIndices())
			.map((index) => fileList.files[index]?.path)
			.filter((path): path is string => Boolean(path)),
	);
	const selectedPath = fileList.files[getSelectedFileIndex()]?.path;
	const nextSortDirection = getSortDirection() === 'ascending' ? 'descending' : 'ascending';
	setSortDirection(nextSortDirection);

	const nextFiles = [...fileList.files];
	// Sort by filename with numeric ordering ("2 -" before "10 -") — the queue's
	// visible identity. Metadata titles must never decide processing order.
	nextFiles.sort((a, b) => {
		const nameA = pathBasename(a.path, { fallback: 'path' });
		const nameB = pathBasename(b.path, { fallback: 'path' });
		const comparison = nameA.localeCompare(nameB, undefined, {
			numeric: true,
			sensitivity: 'base',
		});
		return nextSortDirection === 'ascending' ? comparison : -comparison;
	});
	replaceCurrentFileListFiles(nextFiles);

	setSelectedFileIndices(
		nextFiles.flatMap((file, index) => (selectedPaths.has(file.path) ? [index] : [])),
	);
	setSelectedIndex(selectedPath ? nextFiles.findIndex((file) => file.path === selectedPath) : -1);

	refreshOutputForFileListChange();

	void coordinateMetadataSurfacePresentationRefresh();
}

export function clearAllFiles(): void {
	if (isOrderLocked()) return;
	const fileList = getCurrentFileList();
	if (!fileList) return;
	const inputIds = fileList.files.map((file) => file.inputId);

	clearMetadataSession();
	resetImportOrder([]);
	fileListSessionState.currentFileList = {
		...fileList,
		files: [],
		validCount: 0,
		invalidCount: 0,
		totalDuration: 0,
		totalSize: 0,
	};

	applySelectionIntentToState({ type: 'clear' });
	setSelectedIndex(-1);
	clearSelectionPanels();
	refreshOutputForFileListChange();
	void purgeRemoteSourceSessionsForInputIds(inputIds);
}

export function setFileOrderLocked(locked: boolean): void {
	setOrderLocked(locked);
}

export function reorderFiles(fromIndex: number, toIndex: number): void {
	if (isOrderLocked()) return;
	const fileList = getCurrentFileList();
	if (!fileList) return;

	const nextFiles = [...fileList.files];
	const [moved] = nextFiles.splice(fromIndex, 1);
	nextFiles.splice(toIndex, 0, moved);
	replaceCurrentFileListFiles(nextFiles);
	setSortDirection('none');

	reindexSelectionAfterMove(fromIndex, toIndex);

	refreshOutputForFileListChange();

	void coordinateMetadataSurfacePresentationRefresh();
}

/**
 * Restores the queue to import (arrival) order — the one-click inverse of
 * filename ordering and manual drags. No-op when any file lacks an ordinal
 * (lists seeded outside display/append flows).
 */
export async function restoreImportOrder(): Promise<void> {
	if (isOrderLocked()) return;
	const fileList = getCurrentFileList();
	if (!fileList || fileList.files.length <= 1) return;
	if (fileList.files.some((file) => getImportOrdinal(file.path) === undefined)) return;

	if (
		!(await preserveMetadataDraftsBeforeSelectionChange({
			validationFailureMessage: 'Fix metadata validation errors before restoring import order.',
		}))
	) {
		return;
	}

	const selectedPaths = new Set(
		Array.from(getSelectedFileIndices())
			.map((index) => fileList.files[index]?.path)
			.filter((path): path is string => Boolean(path)),
	);
	const selectedPath = fileList.files[getSelectedFileIndex()]?.path;

	const nextFiles = [...fileList.files].sort(
		(a, b) => (getImportOrdinal(a.path) ?? 0) - (getImportOrdinal(b.path) ?? 0),
	);
	replaceCurrentFileListFiles(nextFiles);
	setSortDirection('none');

	setSelectedFileIndices(
		nextFiles.flatMap((file, index) => (selectedPaths.has(file.path) ? [index] : [])),
	);
	setSelectedIndex(selectedPath ? nextFiles.findIndex((file) => file.path === selectedPath) : -1);

	refreshOutputForFileListChange();

	void coordinateMetadataSurfacePresentationRefresh();
}
