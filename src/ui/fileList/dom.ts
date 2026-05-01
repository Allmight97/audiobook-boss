import { formatFileSize } from '../../types/audio';
import { getCurrentFileList, isOrderLocked, getSelectedFileIndices } from './state.svelte';
import { setFileImportHasFiles } from '../fileImport/state.svelte';
import {
	resetFileListViewState,
	setFileListCombinedSizeText,
	setFileListControlsState,
	setFileListDragState,
	setFileListOrderLockVisible,
	setFileListSortLabel,
	setFileListViewFiles,
	setFileListViewSelection,
} from './viewState.svelte';

export function updateFileListDOM(): void {
	const fileList = getCurrentFileList();
	if (!fileList) {
		resetFileListViewState();
		setFileImportHasFiles(false);
		return;
	}

	const hasFiles = fileList.files.length > 0;

	setFileImportHasFiles(hasFiles);
	setFileListViewFiles([...fileList.files]);

	updateButtonVisibility();
	updateTotalStats();
	updateSelection();
}

export function updateButtonVisibility(): void {
	const fileList = getCurrentFileList();
	if (!fileList) return;
	const locked = isOrderLocked();
	const showSortButton = fileList.files.length > 1;
	const showClearButton = fileList.files.length > 0;
	setFileListControlsState({
		showSortButton,
		showClearButton,
		sortDisabled: locked,
		clearDisabled: locked,
	});
}

export function updateTotalStats(): void {
	const fileList = getCurrentFileList();
	if (!fileList) return;
	setFileListCombinedSizeText(formatFileSize(fileList.totalSize));
}

export function updateSelection(): void {
	const selectedIndices = getSelectedFileIndices();
	setFileListViewSelection(selectedIndices);
}

export function updateSortButtonText(ascending: boolean): void {
	setFileListSortLabel(ascending);
}

export function setOrderLockNotice(locked: boolean): void {
	setFileListOrderLockVisible(locked);
}

export function clearContainer(): void {
	setFileListViewFiles([]);
	setFileListViewSelection(new Set<number>());
	setFileListDragState({ draggedIndex: null, hoveredIndex: null });
	setFileImportHasFiles(false);
}

export function showEmptyState(): void {
	clearContainer();
	setFileListControlsState({
		showSortButton: false,
		showClearButton: false,
		sortDisabled: isOrderLocked(),
		clearDisabled: isOrderLocked(),
	});
}
