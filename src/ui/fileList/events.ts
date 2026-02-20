import { getCurrentFileList, isOrderLocked } from './state';
import { isMetadataSaveInProgress } from '../metadataSaveState';
import {
	selectFile,
	removeFile,
	moveFileUp,
	moveFileDown,
	reorderFiles,
	selectAll,
	clearSelectionAction,
} from './actions';

let draggedIndex: number | null = null;

export function onFileListClick(e: Event): void {
	if (isMetadataSaveInProgress()) return;
	const target = e.target as HTMLElement;

	if (target.classList.contains('remove-file-btn')) {
		e.stopPropagation();
		e.preventDefault();
		if (isOrderLocked()) return;
		const index = parseInt(target.dataset.index || '-1', 10);
		if (index >= 0) {
			removeFile(index);
		}
		return;
	}

	if (target.classList.contains('move-up-btn')) {
		e.stopPropagation();
		e.preventDefault();
		if (isOrderLocked()) return;
		const index = parseInt(target.dataset.index || '-1', 10);
		if (index > 0) {
			moveFileUp(index);
		}
		return;
	}

	if (target.classList.contains('move-down-btn')) {
		e.stopPropagation();
		e.preventDefault();
		if (isOrderLocked()) return;
		const index = parseInt(target.dataset.index || '-1', 10);
		const fileList = getCurrentFileList();
		if (index >= 0 && fileList && index < fileList.files.length - 1) {
			moveFileDown(index);
		}
		return;
	}

	const fileItem = target.closest('.file-list-item') as HTMLElement | null;
	if (!fileItem) {
		return;
	}

	const index = parseInt(fileItem.dataset.index || '-1', 10);
	if (index < 0) {
		return;
	}

	const mouseEvent = e as MouseEvent;
	const multi = mouseEvent.ctrlKey || mouseEvent.metaKey;
	const range = mouseEvent.shiftKey;

	if (range) {
		window.getSelection()?.removeAllRanges();
	}

	void selectFile(index, { multi, range });
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

export function onFileListDragStart(e: DragEvent): void {
	if (isMetadataSaveInProgress()) return;
	if (isOrderLocked()) return;
	if (!e.dataTransfer) return;

	const target = e.target as HTMLElement | null;
	const item = target?.closest('.file-list-item') as HTMLElement | null;
	if (!item) {
		return;
	}

	const index = parseInt(item.dataset.index || '-1', 10);
	if (index < 0) {
		return;
	}

	draggedIndex = index;
	e.dataTransfer.effectAllowed = 'move';
	e.dataTransfer.setData('text/plain', index.toString());
	item.classList.add('dragging');
}

export function onFileListDragOver(e: DragEvent): void {
	if (isMetadataSaveInProgress()) return;
	if (isOrderLocked()) return;
	e.preventDefault();
	if (!e.dataTransfer) return;

	e.dataTransfer.dropEffect = 'move';

	const container = e.currentTarget as HTMLElement;
	const items = Array.from(container.querySelectorAll('.file-list-item'));
	items.forEach((item) => {
		item.classList.remove('drag-over');
	});

	const target = e.target as HTMLElement;
	const fileItem = target.closest('.file-list-item') as HTMLElement;
	if (fileItem && !fileItem.classList.contains('dragging')) {
		fileItem.classList.add('drag-over');
	}
}

export function onFileListDrop(e: DragEvent): void {
	if (isMetadataSaveInProgress()) return;
	if (isOrderLocked()) return;
	e.preventDefault();
	e.stopPropagation();

	if (draggedIndex === null) return;

	const container = e.currentTarget as HTMLElement;
	const items = Array.from(container.querySelectorAll('.file-list-item'));
	items.forEach((item) => {
		item.classList.remove('drag-over');
	});

	const target = e.target as HTMLElement;
	const dropTarget = target.closest('.file-list-item') as HTMLElement | null;
	if (!dropTarget) {
		return;
	}

	const dropIndex = parseInt(dropTarget.dataset.index || '-1', 10);
	if (dropIndex < 0 || dropIndex === draggedIndex) {
		return;
	}

	reorderFiles(draggedIndex, dropIndex);
	draggedIndex = null;
}

export function onFileListDragEnd(e: DragEvent): void {
	const container = e.currentTarget as HTMLElement;
	const draggedItem = container.querySelector('.file-list-item.dragging');
	if (draggedItem) {
		draggedItem.classList.remove('dragging');
	}

	container.querySelectorAll('.file-list-item').forEach((item) => {
		item.classList.remove('drag-over');
	});

	draggedIndex = null;
}

// Temporary no-op exports while modules finish migrating to island-owned listeners.
export function initFileListEvents(): void {}

export function setupDragStartHandlers(): void {}

export function initDOMEventHandlers(): void {}
