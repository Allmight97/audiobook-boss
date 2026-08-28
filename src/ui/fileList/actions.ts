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
	captureFileListMutationSnapshot,
	canCommitFileListMutation,
	findFileIndexByIdentityKey,
	fileIdentityKey,
	replaceFileListFiles,
	getSelectedFiles,
} from './state.svelte';
import {
	clearSelection,
	handleSelection,
	reindexSelectionAfterMove,
	reindexSelectionAfterRemoval,
	selectAllFiles,
	swapSelectionIndices,
} from './selection';
import {
	buildFileListAppendResult,
	normalizeFileListInfo,
	type FileListAppendResult,
} from './appendResult';
import {
	preserveMetadataDraftsBeforeSelectionChange,
	prepareMetadataDraftsForCurrentSelection,
	commitPreparedMetadataDrafts,
	captureMetadataEditSnapshot,
	isCurrentMetadataEditSnapshot,
} from './metadataStaging';
import { purgeRemoteSourceSessionsForInputIds } from '../remoteSource';
import { pathBasename } from '../../lib/path/basename';
import {
	clearFileListCoverThumbnails,
	removeFileListCoverThumbnail,
	scheduleFileListCoverThumbnails,
} from './coverThumbnails.svelte';

function refreshOutputForFileListChange(): void {
	updateEstimatedSize();
}

function setTransientStatusMessage(message: string, timeoutMs: number = 2000): void {
	pushStatusPanelTransientStatus(message, { ttlMs: timeoutMs });
}

// Selection and order actions may await metadata validation or hydration. A
// later user intent supersedes an earlier one before its async preparation can
// commit. The synchronous commit remains guarded by the file-list snapshot.
let latestFileListTransitionId = 0;

function beginFileListTransition(): number {
	latestFileListTransitionId += 1;
	return latestFileListTransitionId;
}

function isCurrentFileListTransition(transitionId: number): boolean {
	return transitionId === latestFileListTransitionId;
}

function selectSoleImportedFile(fileList: FileListInfo): void {
	if (fileList.files.length !== 1) return;
	if (!fileList.files[0]?.isValid) return;

	setSelectedIndex(0);
	setSelectedFileIndices([0]);
}

export function displayFileList(fileListInfo: FileListInfo): void {
	const previousInputIds = getCurrentFileList()?.files.map((file) => file.inputId) ?? [];
	const normalizedFileListInfo = normalizeFileListInfo(fileListInfo);

	clearMetadataSession();
	setCurrentFileList(normalizedFileListInfo);
	resetImportOrder(normalizedFileListInfo.files);

	refreshOutputForFileListChange();

	selectSoleImportedFile(normalizedFileListInfo);
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
	modifiers?: { multi: boolean; range: boolean },
	options?: { skipPersistPrevious?: boolean },
): Promise<void> {
	const intent: SelectionIntent = modifiers?.range
		? { type: 'range', index }
		: modifiers?.multi
			? { type: 'toggle', index }
			: { type: 'selectOnly', index };
	return applySelectionIntent(intent, { skipPersistPrevious: options?.skipPersistPrevious });
}

export type SelectionIntent =
	| { type: 'selectOnly'; index: number }
	| { type: 'toggle'; index: number }
	| { type: 'range'; index: number }
	| { type: 'selectAll' }
	| { type: 'clear' };

export async function applySelectionIntent(
	intent: SelectionIntent,
	options?: { skipPersistPrevious?: boolean },
): Promise<void> {
	const fileList = getCurrentFileList();
	if (!fileList) return;
	const transitionId = beginFileListTransition();
	const mutationSnapshot = captureFileListMutationSnapshot();
	const isCurrent = (): boolean =>
		isCurrentFileListTransition(transitionId) && canCommitFileListMutation(mutationSnapshot);
	if (
		!(await preserveMetadataDraftsBeforeSelectionChange({
			skipSingleSelection: options?.skipPersistPrevious,
			validationFailureMessage: 'Fix metadata validation errors before changing selection.',
			isCurrent,
		}))
	)
		return;
	if (!isCurrent()) return;
	const selectionResult =
		intent.type === 'selectAll'
			? { changed: selectAllFiles() }
			: intent.type === 'clear'
				? { changed: clearSelection() }
				: handleSelection(intent.index, {
						multi: intent.type === 'toggle',
						range: intent.type === 'range',
					});
	if (!selectionResult.changed) return;
	if (getSelectedFiles().length === 0) {
		setSelectedIndex(-1);
	}
}

export async function selectAll(): Promise<void> {
	const fileList = getCurrentFileList();
	if (!fileList) return;
	const transitionId = beginFileListTransition();
	const mutationSnapshot = captureFileListMutationSnapshot();
	const isCurrent = (): boolean =>
		isCurrentFileListTransition(transitionId) && canCommitFileListMutation(mutationSnapshot);

	if (
		!(await preserveMetadataDraftsBeforeSelectionChange({
			validationFailureMessage: 'Fix metadata validation errors before selecting all files.',
			isCurrent,
		}))
	) {
		return;
	}
	if (!isCurrent()) return;

	const changed = selectAllFiles();
	if (!changed) return;
}

export async function clearSelectionAction(): Promise<void> {
	const transitionId = beginFileListTransition();
	const mutationSnapshot = captureFileListMutationSnapshot();
	const isCurrent = (): boolean =>
		isCurrentFileListTransition(transitionId) && canCommitFileListMutation(mutationSnapshot);
	if (
		!(await preserveMetadataDraftsBeforeSelectionChange({
			validationFailureMessage: 'Fix metadata validation errors before clearing the selection.',
			isCurrent,
		}))
	) {
		return;
	}
	if (!isCurrent()) return;

	const changed = clearSelection();
	if (!changed) return;
}

export async function removeFile(index: number): Promise<void> {
	if (isOrderLocked()) return;
	const fileList = getCurrentFileList();
	if (!fileList || index < 0 || index >= fileList.files.length) {
		return;
	}

	const removedFile = fileList.files[index];
	removeFileListCoverThumbnail(removedFile.path);
	removeMetadataForFile(removedFile.path);
	removeImportOrdinal(removedFile.path);
	void purgeRemoteSourceSessionsForInputIds([removedFile.inputId]);

	const nextFiles = [...fileList.files];
	nextFiles.splice(index, 1);
	replaceFileListFiles(nextFiles);
	scheduleFileListCoverThumbnails(
		nextFiles.filter((file) => file.isValid).map((file) => file.path),
	);
	recalculateTotals();

	reindexSelectionAfterRemoval(index);

	refreshOutputForFileListChange();
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

/** Prepare may await; list/lock drift is revalidated before and after the
 * synchronous metadata commit. No cache or intent write happens before the
 * final validation. */
async function prepareRevalidateCommitMetadata(options: {
	validationFailureMessage: string;
	isCurrent?: () => boolean;
}): Promise<boolean> {
	const snapshot = captureFileListMutationSnapshot();
	const editSnapshot = captureMetadataEditSnapshot();
	const prepared = await prepareMetadataDraftsForCurrentSelection({
		validationFailureMessage: options.validationFailureMessage,
		isCurrent: options.isCurrent,
	});
	if (
		!prepared.ok ||
		options.isCurrent?.() === false ||
		!canCommitFileListMutation(snapshot) ||
		!isCurrentMetadataEditSnapshot(editSnapshot)
	)
		return false;
	if (!commitPreparedMetadataDrafts(prepared.prepared, editSnapshot)) return false;
	return options.isCurrent?.() !== false && canCommitFileListMutation(snapshot);
}

export async function removeSelectedFiles(): Promise<void> {
	if (isOrderLocked()) return;
	const fileList = getCurrentFileList();
	if (!fileList) return;
	const transitionId = beginFileListTransition();
	const selectedIdentityKeys = Array.from(getSelectedFileIndices())
		.map((index) => fileList.files[index])
		.filter((file): file is AudioFile => Boolean(file))
		.map((file) => fileIdentityKey(file));
	if (selectedIdentityKeys.length === 0) return;
	if (
		!(await prepareRevalidateCommitMetadata({
			validationFailureMessage: 'Fix metadata validation errors before removing selected files.',
			isCurrent: () => isCurrentFileListTransition(transitionId),
		}))
	) {
		return;
	}
	for (const identityKey of selectedIdentityKeys) {
		const index = findFileIndexByIdentityKey(identityKey);
		if (index >= 0) removeFile(index);
	}
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
	replaceFileListFiles(nextFiles);
	recalculateTotals();
	setSortDirection('none');

	swapSelectionIndices(index, index - 1);

	refreshOutputForFileListChange();
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
	replaceFileListFiles(nextFiles);
	recalculateTotals();
	setSortDirection('none');

	swapSelectionIndices(index, index + 1);

	refreshOutputForFileListChange();
}

export async function toggleFileSort(): Promise<void> {
	if (isOrderLocked()) return;
	const initialList = getCurrentFileList();
	if (!initialList || initialList.files.length <= 1) return;
	const transitionId = beginFileListTransition();
	const selectedPaths = new Set(
		Array.from(getSelectedFileIndices())
			.map((index) => initialList.files[index]?.path)
			.filter((path): path is string => Boolean(path)),
	);
	const selectedPath = initialList.files[getSelectedFileIndex()]?.path;
	if (
		!(await prepareRevalidateCommitMetadata({
			validationFailureMessage: 'Fix metadata validation errors before sorting files.',
			isCurrent: () => isCurrentFileListTransition(transitionId),
		}))
	) {
		return;
	}
	const fileList = getCurrentFileList();
	if (!fileList || fileList.files.length <= 1) return;
	const nextSortDirection = getSortDirection() === 'ascending' ? 'descending' : 'ascending';
	setSortDirection(nextSortDirection);

	const nextFiles = [...fileList.files];
	nextFiles.sort((a, b) => {
		const nameA = pathBasename(a.path, { fallback: 'path' });
		const nameB = pathBasename(b.path, { fallback: 'path' });
		const comparison = nameA.localeCompare(nameB, undefined, {
			numeric: true,
			sensitivity: 'base',
		});
		return nextSortDirection === 'ascending' ? comparison : -comparison;
	});
	replaceFileListFiles(nextFiles);
	recalculateTotals();

	setSelectedFileIndices(
		nextFiles.flatMap((file, index) => (selectedPaths.has(file.path) ? [index] : [])),
	);
	setSelectedIndex(selectedPath ? nextFiles.findIndex((file) => file.path === selectedPath) : -1);

	refreshOutputForFileListChange();
}

export function clearAllFiles(): void {
	if (isOrderLocked()) return;
	const fileList = getCurrentFileList();
	if (!fileList) return;
	const inputIds = fileList.files.map((file) => file.inputId);
	clearFileListCoverThumbnails();

	clearMetadataSession();
	resetImportOrder([]);
	replaceFileListFiles([]);
	recalculateTotals();

	clearSelection();
	setSelectedIndex(-1);
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
	replaceFileListFiles(nextFiles);
	recalculateTotals();
	setSortDirection('none');

	reindexSelectionAfterMove(fromIndex, toIndex);

	refreshOutputForFileListChange();
}

/** Restore the original arrival order captured by display/append. */
export async function restoreImportOrder(): Promise<void> {
	if (isOrderLocked()) return;
	const initialList = getCurrentFileList();
	if (!initialList || initialList.files.length <= 1) return;
	if (initialList.files.some((file) => getImportOrdinal(file.path) === undefined)) return;
	const transitionId = beginFileListTransition();
	const selectedPaths = new Set(
		Array.from(getSelectedFileIndices())
			.map((index) => initialList.files[index]?.path)
			.filter((path): path is string => Boolean(path)),
	);
	const selectedPath = initialList.files[getSelectedFileIndex()]?.path;
	if (
		!(await prepareRevalidateCommitMetadata({
			validationFailureMessage: 'Fix metadata validation errors before restoring import order.',
			isCurrent: () => isCurrentFileListTransition(transitionId),
		}))
	) {
		return;
	}
	const fileList = getCurrentFileList();
	if (!fileList || fileList.files.length <= 1) return;
	if (fileList.files.some((file) => getImportOrdinal(file.path) === undefined)) return;
	const nextFiles = [...fileList.files].sort(
		(a, b) => (getImportOrdinal(a.path) ?? 0) - (getImportOrdinal(b.path) ?? 0),
	);
	replaceFileListFiles(nextFiles);
	recalculateTotals();
	setSortDirection('none');
	setSelectedFileIndices(
		nextFiles.flatMap((file, index) => (selectedPaths.has(file.path) ? [index] : [])),
	);
	setSelectedIndex(selectedPath ? nextFiles.findIndex((file) => file.path === selectedPath) : -1);
	refreshOutputForFileListChange();
}
