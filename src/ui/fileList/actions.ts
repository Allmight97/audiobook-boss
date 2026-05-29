import type { AudioFile, FileListInfo } from '../../types/audio';
import { updateEstimatedSize } from '../outputPanel';
import { pushStatusPanelTransientStatus } from '../statusPanel';
import { clearMetadataState, removeMetadataForFile } from '../metadataState';
import {
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
	updateFileListDOM,
	updateTotalStats,
	updateSelection,
	updateSortButtonText,
	updateButtonVisibility,
	showEmptyState,
	setOrderLockNotice,
} from './dom';
import {
	clearSelection,
	handleSelection,
	reindexSelectionAfterMove,
	reindexSelectionAfterRemoval,
	selectAllFiles,
	swapSelectionIndices,
} from './selection';
import {
	autoUpdateCoverArtFromFirstValidFile,
	clearSelectionPanels,
	getSelectedFiles,
	showMultiSelection,
	showSingleSelection,
} from './metadataPanel';
import {
	buildFileListAppendResult,
	normalizeFileListInfo,
	type FileListAppendResult,
} from './appendResult';
import { persistSingleSelectionMetadata, stageMetadataToSelection } from './metadataStaging';

function refreshOutputForFileListChange(): void {
	updateEstimatedSize();
}

function setTransientStatusMessage(message: string, timeoutMs: number = 2000): void {
	pushStatusPanelTransientStatus(message, { ttlMs: timeoutMs });
}

function setStatusMessage(message: string): void {
	pushStatusPanelTransientStatus(message, { ttlMs: 2_500 });
}

function selectSoleImportedFile(fileList: FileListInfo): void {
	if (fileList.files.length !== 1) return;
	if (!fileList.files[0]?.isValid) return;

	setSelectedIndex(0);
	setSelectedFileIndices([0]);
	updateSelection();
	void showSingleSelection(fileList.files[0]);
}

export function displayFileList(fileListInfo: FileListInfo): void {
	const normalizedFileListInfo = normalizeFileListInfo(fileListInfo);

	clearMetadataState();
	setCurrentFileList(normalizedFileListInfo);
	clearSelectionPanels();

	updateFileListDOM();

	updateTotalStats();
	updateButtonVisibility();
	updateSortButtonText(getSortAscending());

	refreshOutputForFileListChange();

	selectSoleImportedFile(normalizedFileListInfo);
	void autoUpdateCoverArtFromFirstValidFile();
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

	updateFileListDOM();
	updateTotalStats();
	updateButtonVisibility();
	updateSortButtonText(getSortAscending());

	refreshOutputForFileListChange();
	return appendResult;
}

export async function selectFile(
	index: number,
	modifiers?: { multi: boolean; range: boolean },
	options?: { skipPersistPrevious?: boolean },
): Promise<void> {
	const fileList = getCurrentFileList();
	if (!fileList || index < 0 || index >= fileList.files.length) {
		return;
	}

	const previousSelectionCount = getSelectedFiles().length;
	const previousSelectedFiles = previousSelectionCount > 1 ? getSelectedFiles() : [];
	const previousIndex = getSelectedFileIndex();
	const previousFile =
		previousSelectionCount === 1 ? (fileList.files[previousIndex] ?? null) : null;

	if (previousSelectionCount === 1 && previousIndex >= 0 && !options?.skipPersistPrevious) {
		if (!(await persistSingleSelectionMetadata(previousFile))) {
			return;
		}
	}

	if (previousSelectionCount > 1) {
		const didStage = await stageMetadataToSelection({
			showStatus: false,
			selectedFilesOverride: previousSelectedFiles,
		});
		if (!didStage) {
			setStatusMessage('Fix metadata validation errors before changing selection.');
			return;
		}
	}

	const selectionResult = handleSelection(index, modifiers || { multi: false, range: false });
	if (!selectionResult.changed) return;

	updateSelection();

	const selectedFiles = getSelectedFiles();
	const count = selectedFiles.length;

	if (count === 0) {
		setSelectedIndex(-1);
		clearSelectionPanels();
		return;
	}

	if (count === 1) {
		void showSingleSelection(selectedFiles[0]);
		return;
	}

	void showMultiSelection(selectedFiles);
}

export async function selectAll(): Promise<void> {
	const fileList = getCurrentFileList();
	if (!fileList) return;
	const selectedIndex = getSelectedFileIndex();
	const previousFile =
		getSelectedFiles().length === 1 && selectedIndex >= 0
			? (fileList.files[selectedIndex] ?? null)
			: null;
	if (!(await persistSingleSelectionMetadata(previousFile))) {
		return;
	}

	const changed = selectAllFiles();
	if (!changed) return;

	updateSelection();
	const selectedFiles = getSelectedFiles();
	if (selectedFiles.length > 1) {
		void showMultiSelection(selectedFiles);
	} else if (selectedFiles.length === 1) {
		void showSingleSelection(selectedFiles[0]);
	}
}

export async function clearSelectionAction(): Promise<void> {
	const fileList = getCurrentFileList();
	const selectedIndex = getSelectedFileIndex();
	const previousSelectionCount = getSelectedFiles().length;
	const previousFile =
		fileList && previousSelectionCount === 1 && selectedIndex >= 0
			? (fileList.files[selectedIndex] ?? null)
			: null;
	if (!(await persistSingleSelectionMetadata(previousFile))) {
		return;
	}

	if (previousSelectionCount > 1) {
		const didStage = await stageMetadataToSelection({
			showStatus: false,
			selectedFilesOverride: getSelectedFiles(),
		});
		if (!didStage) {
			setStatusMessage('Fix metadata validation errors before clearing the selection.');
			return;
		}
	}

	const changed = clearSelection();
	if (!changed) return;

	updateSelection();
	clearSelectionPanels();
}

export async function removeFile(index: number): Promise<void> {
	if (isOrderLocked()) return;
	const fileList = getCurrentFileList();
	if (!fileList || index < 0 || index >= fileList.files.length) {
		return;
	}

	const removedFile = fileList.files[index];
	removeMetadataForFile(removedFile.path);

	fileList.files.splice(index, 1);
	fileList.validCount = fileList.files.filter((f) => f.isValid).length;
	fileList.invalidCount = fileList.files.length - fileList.validCount;

	recalculateTotals();
	updateFileListDOM();

	reindexSelectionAfterRemoval(index);
	updateSelection();

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

export function recalculateTotals(): void {
	const fileList = getCurrentFileList();
	if (!fileList) return;

	const validFiles = fileList.files.filter((f) => f.isValid && f.duration && f.size);
	fileList.totalDuration = validFiles.reduce((sum, f) => sum + (f.duration || 0), 0);
	fileList.totalSize = validFiles.reduce((sum, f) => sum + (f.size || 0), 0);
}

export function moveFileUp(index: number): void {
	if (isOrderLocked()) return;
	const fileList = getCurrentFileList();
	if (!fileList || index <= 0 || index >= fileList.files.length) {
		return;
	}

	const temp = fileList.files[index];
	fileList.files[index] = fileList.files[index - 1];
	fileList.files[index - 1] = temp;

	swapSelectionIndices(index, index - 1);

	updateFileListDOM();
	refreshOutputForFileListChange();

	const selectedFiles = getSelectedFiles();
	if (selectedFiles.length === 1) {
		void showSingleSelection(selectedFiles[0]);
	}
}

export function moveFileDown(index: number): void {
	if (isOrderLocked()) return;
	const fileList = getCurrentFileList();
	if (!fileList || index < 0 || index >= fileList.files.length - 1) {
		return;
	}

	const temp = fileList.files[index];
	fileList.files[index] = fileList.files[index + 1];
	fileList.files[index + 1] = temp;

	swapSelectionIndices(index, index + 1);

	updateFileListDOM();
	refreshOutputForFileListChange();

	const selectedFiles = getSelectedFiles();
	if (selectedFiles.length === 1) {
		void showSingleSelection(selectedFiles[0]);
	}
}

export function toggleFileSort(): void {
	if (isOrderLocked()) return;
	const fileList = getCurrentFileList();
	if (!fileList || fileList.files.length <= 1) return;

	setSortAscending(!getSortAscending());

	fileList.files.sort((a, b) => {
		const nameA = a.path.split(/[\\/]/).pop() || a.path;
		const nameB = b.path.split(/[\\/]/).pop() || b.path;

		if (getSortAscending()) {
			return nameA.localeCompare(nameB);
		}
		return nameB.localeCompare(nameA);
	});

	clearSelection();
	setSelectedIndex(-1);
	clearSelectionPanels();

	updateSortButtonText(getSortAscending());
	updateButtonVisibility();

	updateFileListDOM();
	refreshOutputForFileListChange();
}

export function clearAllFiles(): void {
	if (isOrderLocked()) return;
	const fileList = getCurrentFileList();
	if (!fileList) return;

	clearMetadataState();
	fileList.files = [];
	fileList.validCount = 0;
	fileList.invalidCount = 0;
	fileList.totalDuration = 0;
	fileList.totalSize = 0;

	showEmptyState();

	clearSelection();
	setSelectedIndex(-1);
	clearSelectionPanels();
	updateTotalStats();
	updateButtonVisibility();
	refreshOutputForFileListChange();
}

export function setFileOrderLocked(locked: boolean): void {
	setOrderLocked(locked);
	setOrderLockNotice(locked);
	updateButtonVisibility();
	updateFileListDOM();
}

export function reorderFiles(fromIndex: number, toIndex: number): void {
	if (isOrderLocked()) return;
	const fileList = getCurrentFileList();
	if (!fileList) return;

	const files = fileList.files;
	const [moved] = files.splice(fromIndex, 1);
	files.splice(toIndex, 0, moved);

	reindexSelectionAfterMove(fromIndex, toIndex);

	updateFileListDOM();
	refreshOutputForFileListChange();

	const selectedFiles = getSelectedFiles();
	if (selectedFiles.length === 1) {
		void showSingleSelection(selectedFiles[0]);
	} else if (selectedFiles.length > 1) {
		void showMultiSelection(selectedFiles);
	} else {
		clearSelectionPanels();
	}
}
