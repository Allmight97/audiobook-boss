import { get } from 'svelte/store';

import {
	getCurrentFileList,
	getSelectedFileIndex,
	getSelectedFileIndices,
	isOrderLocked,
} from './state.svelte';
import { metadataSaveInProgressStore } from '../metadataSaveState';
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

export type FileListDragState = {
	draggedIndex: number | null;
	hoveredIndex: number | null;
};

export type FileListDragHandlers = {
	onDragStart: (index: number, event: DragEvent) => void;
	onDragOver: (index: number, event: DragEvent) => void;
	onDrop: (index: number, event: DragEvent) => void;
	onDragEnd: () => void;
};

export function createFileListDragHandlers(
	setDragState: (state: FileListDragState) => void,
): FileListDragHandlers {
	let draggedIndex: number | null = null;

	function resetDragState(): void {
		draggedIndex = null;
		setDragState({ draggedIndex: null, hoveredIndex: null });
	}

	return {
		onDragStart(index: number, event: DragEvent) {
			if (get(metadataSaveInProgressStore) || isOrderLocked()) return;
			if (!event.dataTransfer || !hasValidIndex(index)) return;

			const item = event.currentTarget as HTMLElement | null;
			if (!item) return;

			draggedIndex = index;
			event.dataTransfer.effectAllowed = 'move';
			event.dataTransfer.setData('text/plain', index.toString());
			setDragState({ draggedIndex: index, hoveredIndex: null });
		},
		onDragOver(index: number, event: DragEvent) {
			if (get(metadataSaveInProgressStore)) return;
			if (isOrderLocked()) return;
			event.preventDefault();
			if (!event.dataTransfer) return;

			event.dataTransfer.dropEffect = 'move';
			if (!hasValidIndex(index)) return;

			setDragState({
				draggedIndex,
				hoveredIndex: draggedIndex === index ? null : index,
			});
		},
		onDrop(index: number, event: DragEvent) {
			if (get(metadataSaveInProgressStore)) return;
			if (isOrderLocked()) return;
			event.preventDefault();
			event.stopPropagation();

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
		},
		onDragEnd() {
			resetDragState();
		},
	};
}

function hasValidIndex(index: number): boolean {
	const fileList = getCurrentFileList();
	return Boolean(fileList && index >= 0 && index < fileList.files.length);
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
	void removeFile(index);
}

function stopInteraction(event: Event): void {
	event.stopPropagation();
	event.preventDefault();
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
		void selectAll();
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
