import { pathBasename } from '../../lib/path/basename';
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
	getSortAscending,
	setSortAscending,
	isOrderLocked,
	setOrderLocked,
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

	setSortAscending(!getSortAscending());

	const nextFiles = [...fileList.files];
	nextFiles.sort((a, b) => {
		const nameA = pathBasename(a.path, { fallback: 'path' });
		const nameB = pathBasename(b.path, { fallback: 'path' });

		if (getSortAscending()) {
			return nameA.localeCompare(nameB);
		}
		return nameB.localeCompare(nameA);
	});
	replaceCurrentFileListFiles(nextFiles);

	applySelectionIntentToState({ type: 'clear' });
	setSelectedIndex(-1);
	clearSelectionPanels();

	refreshOutputForFileListChange();
}

export function clearAllFiles(): void {
	if (isOrderLocked()) return;
	const fileList = getCurrentFileList();
	if (!fileList) return;
	const inputIds = fileList.files.map((file) => file.inputId);

	clearMetadataSession();
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

	reindexSelectionAfterMove(fromIndex, toIndex);

	refreshOutputForFileListChange();

	void coordinateMetadataSurfacePresentationRefresh();
}
