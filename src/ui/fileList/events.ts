import { getCurrentFileList, isOrderLocked } from './state';
import { isMetadataSaveInProgress } from '../metadataSaveState';
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
let draggedItem: HTMLElement | null = null;
let hoveredItem: HTMLElement | null = null;

function stopInteraction(event: Event): void {
	event.stopPropagation();
	event.preventDefault();
}

function hasValidIndex(index: number): boolean {
	const fileList = getCurrentFileList();
	return Boolean(fileList && index >= 0 && index < fileList.files.length);
}

function resetDragState(): void {
	if (draggedItem) {
		draggedItem.classList.remove('dragging');
	}
	if (hoveredItem) {
		hoveredItem.classList.remove('drag-over');
	}
	draggedItem = null;
	hoveredItem = null;
	draggedIndex = null;
}

export function onFileListClick(index: number, event: MouseEvent): void {
	if (isMetadataSaveInProgress()) return;
	if (!hasValidIndex(index)) return;

	const multi = event.ctrlKey || event.metaKey;
	const range = event.shiftKey;
	if (range) {
		window.getSelection()?.removeAllRanges();
	}

	void selectFile(index, { multi, range });
}

export function onFileListMoveUp(index: number, event: MouseEvent): void {
	if (isMetadataSaveInProgress() || isOrderLocked()) return;
	if (index <= 0 || !hasValidIndex(index)) return;

	stopInteraction(event);
	moveFileUp(index);
}

export function onFileListMoveDown(index: number, event: MouseEvent): void {
	if (isMetadataSaveInProgress() || isOrderLocked()) return;
	if (!hasValidIndex(index)) return;
	const fileList = getCurrentFileList();
	if (!fileList || index >= fileList.files.length - 1) return;

	stopInteraction(event);
	moveFileDown(index);
}

export function onFileListRemove(index: number, event: MouseEvent): void {
	if (isMetadataSaveInProgress() || isOrderLocked()) return;
	if (!hasValidIndex(index)) return;

	stopInteraction(event);
	removeFile(index);
}

export function onFileListKeyDown(e: KeyboardEvent): void {
	if (isMetadataSaveInProgress()) return;
	if (!getCurrentFileList()) return;
	if (isTextInputTarget(e.target)) return;

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
	if (isMetadataSaveInProgress() || isOrderLocked()) return;
	if (!e.dataTransfer || !hasValidIndex(index)) return;

	const item = e.currentTarget as HTMLElement | null;
	if (!item) return;

	draggedIndex = index;
	draggedItem = item;
	e.dataTransfer.effectAllowed = 'move';
	e.dataTransfer.setData('text/plain', index.toString());
	item.classList.add('dragging');
}

export function onFileListDragOver(index: number, e: DragEvent): void {
	if (isMetadataSaveInProgress()) return;
	if (isOrderLocked()) return;
	e.preventDefault();
	if (!e.dataTransfer) return;

	e.dataTransfer.dropEffect = 'move';
	if (!hasValidIndex(index)) return;

	const item = e.currentTarget as HTMLElement | null;
	if (!item) return;

	if (hoveredItem && hoveredItem !== item) {
		hoveredItem.classList.remove('drag-over');
	}
	hoveredItem = item;
	if (!item.classList.contains('dragging')) {
		item.classList.add('drag-over');
	}
}

export function onFileListDrop(index: number, e: DragEvent): void {
	if (isMetadataSaveInProgress()) return;
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
