import type { AudioFile, FileListInfo } from '../../types/audio';
import { updateEstimatedSize, updateOutputPath } from '../outputPanel';
import { pushStatusPanelTransientStatus } from '../statusPanel';
import {
	clearMetadataState,
	getMetadataForFile,
	metadataEqualsNullish,
	removeMetadataForFile,
	setMetadataForFile,
} from '../metadataState';
import {
	applyMetadataIntentPatch,
	buildMetadataIntentPatchFromMetadata,
	hasActionableMetadataIntentPatch,
} from '../../types/metadataIntent';
import {
	getSeriesPartValidationError,
	getSubseriesPartValidationError,
} from '../metadataValidation';
import { hasDirtyMetadataFields, readMetadataForm, resetDirtyState } from '../metadataForm';
import type { AudiobookMetadata } from '../../types/metadata';
import {
	getCurrentFileList,
	getSelectedFileIndex,
	setCurrentFileList,
	setSelectedIndex,
	getSortAscending,
	setSortAscending,
	isOrderLocked,
	setOrderLocked,
} from './state';
import {
	initDOMCache,
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
	ensureMetadataForFiles,
	getSelectedFiles,
	showMultiSelection,
	showSingleSelection,
} from './metadataPanel';

function refreshOutputForFileListChange(): void {
	updateEstimatedSize();
}

function refreshOutputForMetadataChange(): void {
	updateOutputPath();
	updateEstimatedSize();
}

function setStatusMessage(message: string): void {
	pushStatusPanelTransientStatus(message, { ttlMs: 2_500 });
}

function setTransientStatusMessage(message: string, timeoutMs: number = 2000): void {
	pushStatusPanelTransientStatus(message, { ttlMs: timeoutMs });
}

export function displayFileList(fileListInfo: FileListInfo): void {
	clearMetadataState();
	setCurrentFileList(fileListInfo);
	clearSelectionPanels();
	initDOMCache();

	updateFileListDOM();

	updateTotalStats();
	updateButtonVisibility();
	updateSortButtonText(getSortAscending());

	refreshOutputForFileListChange();

	void autoUpdateCoverArtFromFirstValidFile();
}

function validateSeriesFields(changes: Partial<AudiobookMetadata>): string | null {
	const seriesPartError = getSeriesPartValidationError(
		typeof changes.series_part === 'string' ? changes.series_part : undefined,
	);
	const subseriesPartError = getSubseriesPartValidationError(
		typeof changes.subseries_part === 'string' ? changes.subseries_part : undefined,
	);
	return seriesPartError ?? subseriesPartError;
}

function persistSingleSelectionMetadata(file: AudioFile | null): boolean {
	if (!file?.isValid) return false;
	if (!hasDirtyMetadataFields()) return false;

	const metadata = readMetadataForm({ mode: 'single' });
	const validationError = validateSeriesFields(metadata);
	if (validationError) {
		setStatusMessage(validationError);
		return false;
	}

	const existing = getMetadataForFile(file.path) ?? {};
	const intentPatch = buildMetadataIntentPatchFromMetadata(metadata);
	if (!hasActionableMetadataIntentPatch(intentPatch)) {
		return false;
	}
	const merged = applyMetadataIntentPatch(existing, intentPatch);
	if (metadataEqualsNullish(existing, merged)) {
		return false;
	}

	setMetadataForFile(file.path, merged, {
		markPending: true,
		intentPatch,
	});
	resetDirtyState();
	refreshOutputForMetadataChange();
	return true;
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

	const selectionResult = handleSelection(index, modifiers || { multi: false, range: false });
	if (!selectionResult.changed) return;

	if (previousSelectionCount === 1 && previousIndex >= 0 && !options?.skipPersistPrevious) {
		persistSingleSelectionMetadata(previousFile);
	}

	if (previousSelectionCount > 1) {
		await stageMetadataToSelection({
			showStatus: false,
			selectedFilesOverride: previousSelectedFiles,
		});
	}

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

export function selectAll(): void {
	const fileList = getCurrentFileList();
	if (!fileList) return;
	const selectedIndex = getSelectedFileIndex();
	const previousFile =
		getSelectedFiles().length === 1 && selectedIndex >= 0
			? (fileList.files[selectedIndex] ?? null)
			: null;
	persistSingleSelectionMetadata(previousFile);

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
	persistSingleSelectionMetadata(previousFile);

	if (previousSelectionCount > 1) {
		await stageMetadataToSelection({
			showStatus: false,
			selectedFilesOverride: getSelectedFiles(),
		});
	}

	const changed = clearSelection();
	if (!changed) return;

	updateSelection();
	clearSelectionPanels();
}

export async function stageMetadataToSelection(options?: {
	showStatus?: boolean;
	selectedFilesOverride?: AudioFile[];
}): Promise<boolean> {
	if (!getCurrentFileList()) return false;

	const selectedFiles = (options?.selectedFilesOverride ?? getSelectedFiles()).filter(
		(file) => file.isValid,
	);
	if (selectedFiles.length === 0) return false;

	const changes = readMetadataForm({ mode: 'multi', onlyDirty: true });
	if (Object.keys(changes).length === 0) {
		if (options?.showStatus) {
			setStatusMessage('No metadata changes to apply');
		}
		return false;
	}

	const validationError = validateSeriesFields(changes);
	if (validationError) {
		if (options?.showStatus) {
			setStatusMessage(validationError);
		}
		return false;
	}

	await ensureMetadataForFiles(selectedFiles);
	const intentPatch = buildMetadataIntentPatchFromMetadata(changes);
	if (!hasActionableMetadataIntentPatch(intentPatch)) {
		return false;
	}

	selectedFiles.forEach((file) => {
		const existing = getMetadataForFile(file.path) ?? {};
		const merged = applyMetadataIntentPatch(existing, intentPatch);
		if (!metadataEqualsNullish(existing, merged)) {
			setMetadataForFile(file.path, merged, {
				markPending: true,
				intentPatch,
			});
		}
	});

	resetDirtyState();
	refreshOutputForMetadataChange();

	if (options?.showStatus) {
		setTransientStatusMessage(`Draft saved for ${selectedFiles.length} files`);
	}

	return true;
}

export async function persistPendingMetadataDraftsForCurrentSelection(options?: {
	showStatus?: boolean;
}): Promise<boolean> {
	const selectedFiles = getSelectedFiles();
	if (selectedFiles.length === 0) {
		return false;
	}
	if (selectedFiles.length === 1) {
		const changed = persistSingleSelectionMetadata(selectedFiles[0]);
		if (changed && options?.showStatus) {
			setStatusMessage('Draft saved');
		}
		return changed;
	}

	return stageMetadataToSelection({ showStatus: options?.showStatus });
}

export function removeFile(index: number): void {
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
