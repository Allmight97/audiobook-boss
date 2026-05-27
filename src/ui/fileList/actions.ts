import type { AudioFile, FileListInfo } from '../../types/audio';
import { tauriClient } from '../../lib/tauri/client';
import { updateEstimatedSize, updateOutputPath } from '../outputPanel';
import { pushStatusPanelTransientStatus } from '../statusPanel';
import {
	clearMetadataState,
	getMetadataForFile,
	metadataEqualsNullish,
	removeMetadataForFile,
	setMetadataForFile,
} from '../metadataState';
import { applyMetadataDraftIntent, hasActionableMetadataDraftIntent } from '../metadataDraft';
import {
	firstMetadataIntentValidationError,
	validateMetadataDraftIntent,
} from '../metadataValidation';
import { hasDirtyMetadataFields, readMetadataForm, resetDirtyState } from '../metadataForm';
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
	ensureMetadataForFiles,
	getSelectedFiles,
	showMultiSelection,
	showSingleSelection,
} from './metadataPanel';

function refreshOutputForFileListChange(): void {
	updateEstimatedSize();
}

function refreshOutputForMetadataChange(): void {
	updateOutputPath('final');
	updateEstimatedSize();
}

function collectUniqueFiles(files: AudioFile[], seenPaths: Set<string> = new Set()): AudioFile[] {
	const uniqueFiles: AudioFile[] = [];
	for (const file of files) {
		if (seenPaths.has(file.path)) {
			continue;
		}
		seenPaths.add(file.path);
		uniqueFiles.push(file);
	}
	return uniqueFiles;
}

function buildSelectedDecoderByPath(
	fileList: Pick<FileListInfo, 'files' | 'selectedDecoders'>,
): Map<string, FileListInfo['selectedDecoders'][number]> {
	const byPath = new Map<string, FileListInfo['selectedDecoders'][number]>();
	for (const [index, file] of fileList.files.entries()) {
		byPath.set(file.path, fileList.selectedDecoders[index] ?? null);
	}
	return byPath;
}

function refreshDerivedFileListState(fileList: FileListInfo): void {
	if (fileList.selectedDecoders.length !== fileList.files.length) {
		const decoderByPath = buildSelectedDecoderByPath(fileList);
		fileList.selectedDecoders = fileList.files.map((file) => decoderByPath.get(file.path) ?? null);
	}
	fileList.validCount = fileList.files.filter((file) => file.isValid).length;
	fileList.invalidCount = fileList.files.length - fileList.validCount;
	recalculateTotals();
}

function buildFileListInfoFromFiles(
	files: AudioFile[],
	decoderByPath: Map<string, FileListInfo['selectedDecoders'][number]> = new Map(),
): FileListInfo {
	const uniqueFiles = collectUniqueFiles(files);
	const fileList = {
		files: uniqueFiles,
		selectedDecoders: uniqueFiles.map((file) => decoderByPath.get(file.path) ?? null),
		totalDuration: 0,
		totalSize: 0,
		validCount: 0,
		invalidCount: 0,
	};
	refreshDerivedFileListState(fileList);
	return fileList;
}

function setStatusMessage(message: string): void {
	pushStatusPanelTransientStatus(message, { ttlMs: 2_500 });
}

function setTransientStatusMessage(message: string, timeoutMs: number = 2000): void {
	pushStatusPanelTransientStatus(message, { ttlMs: timeoutMs });
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
	const uniqueFiles = collectUniqueFiles(fileListInfo.files);
	const decoderByPath = buildSelectedDecoderByPath(fileListInfo);
	const normalizedFileListInfo =
		uniqueFiles.length === fileListInfo.files.length
			? fileListInfo
			: {
					...fileListInfo,
					files: uniqueFiles,
					selectedDecoders: uniqueFiles.map((file) => decoderByPath.get(file.path) ?? null),
				};
	refreshDerivedFileListState(normalizedFileListInfo);

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
): void {
	const currentFileList = getCurrentFileList();
	const existingFiles = options?.existingFiles ?? currentFileList?.files ?? [];
	if (existingFiles.length === 0) {
		displayFileList(fileListInfo);
		return;
	}

	const appendedFiles = collectUniqueFiles(
		fileListInfo.files,
		new Set(existingFiles.map((file) => file.path)),
	);

	if (appendedFiles.length === 0) {
		if (options?.showDuplicateStatus ?? true) {
			setTransientStatusMessage('No new files added. All analyzed files were already in the list.');
		}
		return;
	}

	const selectedIndex = getSelectedFileIndex();
	const selectedIndices = getSelectedFileIndices();
	const decoderByPath = new Map<string, FileListInfo['selectedDecoders'][number]>();
	if (currentFileList) {
		for (const [path, selection] of buildSelectedDecoderByPath(currentFileList)) {
			decoderByPath.set(path, selection);
		}
	}
	for (const [path, selection] of buildSelectedDecoderByPath(fileListInfo)) {
		decoderByPath.set(path, selection);
	}
	const mergedFileList = buildFileListInfoFromFiles(
		[...existingFiles, ...appendedFiles],
		decoderByPath,
	);
	setCurrentFileList(mergedFileList);
	setSelectedIndex(selectedIndex);
	setSelectedFileIndices(selectedIndices);

	updateFileListDOM();
	updateTotalStats();
	updateButtonVisibility();
	updateSortButtonText(getSortAscending());

	refreshOutputForFileListChange();
}

async function persistSingleSelectionMetadata(file: AudioFile | null): Promise<boolean> {
	if (!file?.isValid) return true;
	if (!hasDirtyMetadataFields()) return true;

	const metadata = readMetadataForm({ mode: 'single' });
	const validation = await validateMetadataDraftIntent(
		metadata,
		tauriClient.validateMetadataIntentPatch,
	);
	const validationError = firstMetadataIntentValidationError(validation.result);
	if (validationError) {
		setStatusMessage(validationError);
		return false;
	}

	const existing = getMetadataForFile(file.path) ?? {};
	const intentPatch = validation.intentPatch;
	if (!hasActionableMetadataDraftIntent(intentPatch)) {
		return true;
	}
	const merged = applyMetadataDraftIntent(existing, intentPatch);
	if (metadataEqualsNullish(existing, merged)) {
		return true;
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

export async function stageMetadataToSelection(options?: {
	showStatus?: boolean;
	selectedFilesOverride?: AudioFile[];
}): Promise<boolean> {
	if (!getCurrentFileList()) return true;

	const selectedFiles = (options?.selectedFilesOverride ?? getSelectedFiles()).filter(
		(file) => file.isValid,
	);
	if (selectedFiles.length === 0) return true;

	const changes = readMetadataForm({ mode: 'multi', onlyDirty: true });
	if (Object.keys(changes).length === 0) {
		if (options?.showStatus) {
			setStatusMessage('No metadata changes to apply');
		}
		return true;
	}

	const validation = await validateMetadataDraftIntent(
		changes,
		tauriClient.validateMetadataIntentPatch,
	);
	const validationError = firstMetadataIntentValidationError(validation.result);
	if (validationError) {
		if (options?.showStatus) {
			setStatusMessage(validationError);
		}
		return false;
	}

	await ensureMetadataForFiles(selectedFiles);
	const intentPatch = validation.intentPatch;
	if (!hasActionableMetadataDraftIntent(intentPatch)) {
		return true;
	}

	selectedFiles.forEach((file) => {
		const existing = getMetadataForFile(file.path) ?? {};
		const merged = applyMetadataDraftIntent(existing, intentPatch);
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
	const selectedFiles = getSelectedFiles().filter((file) => file.isValid);
	if (selectedFiles.length === 0) {
		return true;
	}
	if (selectedFiles.length === 1) {
		const hadDirtyMetadata = hasDirtyMetadataFields();
		const persisted = await persistSingleSelectionMetadata(selectedFiles[0]);
		if (persisted && hadDirtyMetadata && options?.showStatus) {
			setStatusMessage('Draft saved');
		}
		return persisted;
	}

	return stageMetadataToSelection({ showStatus: options?.showStatus });
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
