import { type AudioFile, formatDuration, formatFileSize } from '../../types/audio';
import { getCurrentFileList, isOrderLocked, getSelectedFileIndices } from './state';
import { setFileImportHasFiles } from '../fileImport/state.svelte';
import {
	resetFileListViewState,
	setFileListControlsState,
	setFileListOrderLockVisible,
	setFileListSortLabel,
	setFileListViewFiles,
	setFileListViewSelection,
} from './viewState.svelte';

export function initDOMCache(): void {
	// Intentionally a no-op after Svelte list render cutover.
}

export function createFileListItem(file: AudioFile, index: number): HTMLElement {
	const item = document.createElement('div');
	item.className = `file-list-item ${file.isValid ? 'valid' : 'invalid'}`;
	item.dataset.index = index.toString();
	item.setAttribute('draggable', isOrderLocked() ? 'false' : 'true');
	item.setAttribute('role', 'listitem');

	const fileName = file.path.split(/[\\/]/).pop() || file.path;
	item.setAttribute('aria-label', fileName);
	const statusIcon = file.isValid ? '✓' : '✗';
	const statusClass = file.isValid ? 'text-green-500' : 'text-red-500';

	const isFirst = index === 0;
	const fileList = getCurrentFileList();
	const isLast = fileList ? index === fileList.files.length - 1 : false;

	const locked = isOrderLocked();
	item.innerHTML = `
        <div class="file-item-content">
            <div class="file-status ${statusClass}">${statusIcon}</div>
            <div class="file-info">
                <div class="file-name">${fileName}</div>
                <div class="file-details">
                    ${
											file.isValid && file.duration && file.size
												? `${formatDuration(file.duration)} • ${formatFileSize(
														file.size,
													)} • ${file.format}`
												: `Error: ${file.error || 'Invalid file'}`
										}
                </div>
            </div>
            <button class="move-up-btn" data-index="${index}" ${
							isFirst || locked ? 'disabled' : ''
						}>▲</button>
            <button class="move-down-btn" data-index="${index}" ${
							isLast || locked ? 'disabled' : ''
						}>▼</button>
            <button class="remove-file-btn" data-index="${index}" ${
							locked ? 'disabled' : ''
						}>×</button>
        </div>
    `;

	return item;
}

export function updateFileListItem(item: HTMLElement, file: AudioFile, index: number): void {
	item.className = `file-list-item ${file.isValid ? 'valid' : 'invalid'}`;
	item.dataset.index = index.toString();
	item.setAttribute('draggable', isOrderLocked() ? 'false' : 'true');
	item.setAttribute('role', 'listitem');

	const fileName = file.path.split(/[\\/]/).pop() || file.path;
	item.setAttribute('aria-label', fileName);
	const statusIcon = file.isValid ? '✓' : '✗';
	const statusClass = file.isValid ? 'text-green-500' : 'text-red-500';

	const isFirst = index === 0;
	const fileList = getCurrentFileList();
	const isLast = fileList ? index === fileList.files.length - 1 : false;

	const locked = isOrderLocked();
	item.innerHTML = `
        <div class="file-item-content">
            <div class="file-status ${statusClass}">${statusIcon}</div>
            <div class="file-info">
                <div class="file-name">${fileName}</div>
                <div class="file-details">
                    ${
											file.isValid && file.duration && file.size
												? `${formatDuration(file.duration)} • ${formatFileSize(
														file.size,
													)} • ${file.format}`
												: `Error: ${file.error || 'Invalid file'}`
										}
                </div>
            </div>
            <button class="move-up-btn" data-index="${index}" ${
							isFirst || locked ? 'disabled' : ''
						}>▲</button>
            <button class="move-down-btn" data-index="${index}" ${
							isLast || locked ? 'disabled' : ''
						}>▼</button>
            <button class="remove-file-btn" data-index="${index}" ${
							locked ? 'disabled' : ''
						}>×</button>
        </div>
    `;
}

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

	const sortButton = document.getElementById('sort-toggle-btn') as HTMLButtonElement | null;
	if (sortButton) {
		sortButton.style.display = showSortButton ? 'block' : 'none';
		sortButton.disabled = locked;
	}
	const clearButton = document.getElementById('clear-files-btn') as HTMLButtonElement | null;
	if (clearButton) {
		clearButton.style.display = showClearButton ? 'block' : 'none';
		clearButton.disabled = locked;
	}
}

export function updateTotalStats(): void {
	const fileList = getCurrentFileList();
	if (!fileList) return;

	const totalSizeEl = document.getElementById('prop-combinedsize');
	if (totalSizeEl) totalSizeEl.textContent = formatFileSize(fileList.totalSize);
}

export function updateSelection(): void {
	const selectedIndices = getSelectedFileIndices();
	setFileListViewSelection(selectedIndices);
}

export function updateSortButtonText(ascending: boolean): void {
	setFileListSortLabel(ascending);
	const sortButton = document.getElementById('sort-toggle-btn') as HTMLButtonElement | null;
	if (sortButton) {
		sortButton.textContent = ascending ? 'Sort: A-Z' : 'Sort: Z-A';
	}
}

export function setOrderLockNotice(locked: boolean): void {
	setFileListOrderLockVisible(locked);
	const notice = document.getElementById('file-order-lock');
	if (notice) {
		notice.style.display = locked ? 'inline' : 'none';
	}
}

export function clearContainer(): void {
	setFileListViewFiles([]);
	setFileListViewSelection(new Set<number>());
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
