import { get } from 'svelte/store';

import {
	getCurrentFileList,
	getSelectedFileIndex,
	getSelectedFileIndices,
	isOrderLocked,
} from './state.svelte';
import { metadataSaveInProgressStore } from '../metadataSaveState';
import { setFileListDragState } from './viewState.svelte';
import {
	fileListNavigationCommandFromKey,
	resolveFileListNavigationTarget,
} from './keyboardNavigation';
import {
	clearSelectionAction,
	moveFileDown,
	moveFileUp,
	removeFile,
	reorderFiles,
	selectAll,
	selectFile,
} from './actions';

let draggedIndex: number | null = null;

function stopInteraction(event: Event): void {
	event.stopPropagation();
	event.preventDefault();
}

function hasValidIndex(index: number): boolean {
	const fileList = getCurrentFileList();
	return Boolean(fileList && index >= 0 && index < fileList.files.length);
}

function resetDragState(): void {
	draggedIndex = null;
	setFileListDragState({ draggedIndex: null, hoveredIndex: null });
}

export function onFileListClick(index: number, event: MouseEvent): void {
	if (get(metadataSaveInProgressStore)) return;
	if (!hasValidIndex(index)) return;

	const multi = event.ctrlKey || event.metaKey;
	const range = event.shiftKey;
	if (range) {
		window.getSelection()?.removeAllRanges();
	}

	void selectFile(index, { multi, range });
}

export function onFileListMoveUp(index: number, event: MouseEvent): void {
	if (get(metadataSaveInProgressStore) || isOrderLocked()) return;
	if (index <= 0 || !hasValidIndex(index)) return;

	stopInteraction(event);
	moveFileUp(index);
}

export function onFileListMoveDown(index: number, event: MouseEvent): void {
	if (get(metadataSaveInProgressStore) || isOrderLocked()) return;
	if (!hasValidIndex(index)) return;
	const fileList = getCurrentFileList();
	if (!fileList || index >= fileList.files.length - 1) return;

	stopInteraction(event);
	moveFileDown(index);
}

export function onFileListRemove(index: number, event: MouseEvent): void {
	if (get(metadataSaveInProgressStore) || isOrderLocked()) return;
	if (!hasValidIndex(index)) return;

	stopInteraction(event);
	removeFile(index);
}

function isCurrentSingleSelection(index: number): boolean {
	const selectedIndices = getSelectedFileIndices();
	return selectedIndices.size === 1 && selectedIndices.has(index);
}

function handleKeyboardNavigation(event: KeyboardEvent): boolean {
	const command = fileListNavigationCommandFromKey(event);
	if (!command) return false;

	const fileList = getCurrentFileList();
	if (!fileList) return false;

	const targetIndex = resolveFileListNavigationTarget({
		command,
		fileCount: fileList.files.length,
		selectedIndex: getSelectedFileIndex(),
	});
	if (targetIndex === null) return false;

	event.preventDefault();
	if (isCurrentSingleSelection(targetIndex)) {
		return true;
	}

	void selectFile(targetIndex, { multi: false, range: false });
	return true;
}

export function onFileListKeyDown(e: KeyboardEvent): void {
	if (get(metadataSaveInProgressStore)) return;
	if (!getCurrentFileList()) return;
	if (isTextInputTarget(e.target)) return;

	if (handleKeyboardNavigation(e)) {
		return;
	}

	const key = e.key.toLowerCase();
	if ((e.metaKey || e.ctrlKey) && key === 'a') {
		e.preventDefault();
		selectAll();
		return;
	}

	if (key === 'escape') {
		e.preventDefault();
		void clearSelectionAction();
	}
}

function isTextInputTarget(target: EventTarget | null): boolean {
	if (!target || !(target instanceof HTMLElement)) return false;
	const tagName = target.tagName.toLowerCase();
	return tagName === 'input' || tagName === 'textarea';
}

export function onFileListDragStart(index: number, e: DragEvent): void {
	if (get(metadataSaveInProgressStore) || isOrderLocked()) return;
	if (!e.dataTransfer || !hasValidIndex(index)) return;

	const item = e.currentTarget as HTMLElement | null;
	if (!item) return;

	draggedIndex = index;
	e.dataTransfer.effectAllowed = 'move';
	e.dataTransfer.setData('text/plain', index.toString());
	setFileListDragState({ draggedIndex: index, hoveredIndex: null });
}

export function onFileListDragOver(index: number, e: DragEvent): void {
	if (get(metadataSaveInProgressStore)) return;
	if (isOrderLocked()) return;
	e.preventDefault();
	if (!e.dataTransfer) return;

	e.dataTransfer.dropEffect = 'move';
	if (!hasValidIndex(index)) return;

	setFileListDragState({
		draggedIndex,
		hoveredIndex: draggedIndex === index ? null : index,
	});
}

export function onFileListDrop(index: number, e: DragEvent): void {
	if (get(metadataSaveInProgressStore)) return;
	if (isOrderLocked()) return;
	e.preventDefault();
	e.stopPropagation();

	if (draggedIndex === null || draggedIndex === index) {
		resetDragState();
		return;
	}
	if (!hasValidIndex(index)) {
		resetDragState();
		return;
	}

	reorderFiles(draggedIndex, index);
	resetDragState();
}

export function onFileListDragEnd(): void {
	resetDragState();
}
